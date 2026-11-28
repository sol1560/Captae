//! 素材库：SQLite + FTS5。
//!
//! 下载完成时自动入库，元数据直接来自下载阶段（标题、简介、来源页面），不额外计算。
//! 全文检索覆盖标题、简介、标签、来源站点和字幕文字，界面搜索框和 agent 的
//! `library_query` 用的是同一套查询。

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS assets (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL,              -- video / audio / image / subtitle / other
  title        TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  source_url   TEXT NOT NULL DEFAULT '',   -- 下载链接
  page_url     TEXT NOT NULL DEFAULT '',   -- 来源页面
  site         TEXT NOT NULL DEFAULT '',   -- YouTube / blender.org …
  uploader     TEXT NOT NULL DEFAULT '',
  path         TEXT NOT NULL UNIQUE,
  format       TEXT NOT NULL DEFAULT '',
  width        INTEGER,
  height       INTEGER,
  duration     REAL,
  size         INTEGER NOT NULL DEFAULT 0,
  thumb        TEXT,
  transcript   TEXT NOT NULL DEFAULT '',   -- 字幕或识别出的文字，参与全文检索
  parent_id    INTEGER REFERENCES assets(id) ON DELETE SET NULL,  -- 转换/剪辑产物指向原素材
  created_at   INTEGER NOT NULL
  -- 后来加的列在 migrate() 里加（老的素材库打开时也加上）：live、category_id、category_by
);
CREATE INDEX IF NOT EXISTS assets_created ON assets(created_at DESC);
CREATE INDEX IF NOT EXISTS assets_kind ON assets(kind);
CREATE INDEX IF NOT EXISTS assets_site ON assets(site);
CREATE TABLE IF NOT EXISTS tags (
  asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  tag      TEXT NOT NULL,
  PRIMARY KEY (asset_id, tag)
  -- 后来加的列：by（user 用户加的 / ai AI 加的）
);
-- 分类（每条素材最多属于一个）：AI 整理时提议、用户确认，或者用户自己建（categorize.rs）
CREATE TABLE IF NOT EXISTS categories (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',   -- 一句话说明，界面上显示，也写进给模型的提示词
  sort        INTEGER NOT NULL DEFAULT 0,
  by          TEXT NOT NULL DEFAULT 'user', -- ai：AI 整理时建的（重新整理时可以换掉）/ user：用户建的（不动）
  created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS tags_tag ON tags(tag);
CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
  title, description, tags, site, transcript,
  tokenize = 'trigram'
);
-- 按素材缓存的小块数据（网站字幕列表等），素材删掉时一起删
CREATE TABLE IF NOT EXISTS asset_cache (
  asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  key      TEXT NOT NULL,
  value    TEXT NOT NULL,
  at       INTEGER NOT NULL,
  PRIMARY KEY (asset_id, key)
);
CREATE TABLE IF NOT EXISTS agent_calls (
  id        INTEGER PRIMARY KEY,
  at        INTEGER NOT NULL,
  client    TEXT NOT NULL,
  tool      TEXT NOT NULL,
  args      TEXT NOT NULL,
  status    TEXT NOT NULL,     -- run / ok / err
  result    TEXT,
  ms        INTEGER
);
"#;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Asset {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub desc: String,
    pub url: String,
    pub page: String,
    /// 来源站点，设计稿里的 `src`
    pub src: String,
    pub uploader: String,
    pub path: PathBuf,
    pub fmt: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "durS")]
    pub duration: Option<f64>,
    pub bytes: u64,
    pub thumb: Option<PathBuf>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub parent_id: Option<i64>,
    pub has_transcript: bool,
    /// 录直播录下来的（和它转换、剪辑出来的）：按链接查「下载过没有」时不算
    pub live: bool,
    /// 分类（categories.id），没有就是「未分类」
    pub category_id: Option<i64>,
    /// 分类是谁定的：空 = 还没整理过 / ai / user（用户亲手放的，AI 不再动；放进「未分类」也算）
    pub category_by: String,
    /// `tags` 里哪些是 AI 加的（界面上样子不一样，能一起清掉）
    pub ai_tags: Vec<String>,
    // ---- 展示用派生字段 ----
    pub res: String,
    pub dur: String,
    pub size: String,
    pub when: String,
    pub day: i64,
    pub exists: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct NewAsset {
    pub kind: String,
    pub title: String,
    pub description: String,
    pub source_url: String,
    pub page_url: String,
    pub site: String,
    pub uploader: String,
    pub path: PathBuf,
    pub format: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<f64>,
    pub size: u64,
    pub thumb: Option<PathBuf>,
    pub tags: Vec<String>,
    pub parent_id: Option<i64>,
    /// 录直播录下来的：同一个直播链接每次录的是不同的内容，插件、AI 助手查「下载过没有」时跳过它（`find_by_url`）
    pub live: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Query {
    /// 关键词：标题、简介、标签、来源、字幕文字（AI 助手常写成 keyword / query，也认）
    #[serde(alias = "keyword", alias = "query")]
    pub q: Option<String>,
    /// video / audio / image
    #[serde(alias = "type")]
    pub kind: Option<String>,
    /// 来源站点，如 YouTube
    #[serde(alias = "source")]
    pub site: Option<String>,
    /// 格式，如 MP4
    pub format: Option<String>,
    /// 最近几天
    pub days: Option<i64>,
    /// ISO 日期 YYYY-MM-DD，下载时间不早于
    pub since: Option<String>,
    pub tags: Vec<String>,
    /// 按分类筛（界面用 id）：0 = 未分类
    #[serde(alias = "category_id")]
    pub cat: Option<i64>,
    /// 按分类名筛（AI 助手用名字）
    pub category: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// 分类和它下面有几条素材
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub desc: String,
    pub sort: i64,
    /// ai / user
    pub by: String,
    pub count: u64,
}

/// 整理时挑哪些素材
#[derive(Debug, Clone)]
pub enum Pick {
    /// 用户没亲手定过分类的（重新整理）
    NotByUser,
    /// 未分类、而且不是用户亲手放进未分类的
    Uncategorized,
    /// 还没整理过的（新下载的、上次没分成的）
    Never,
    Ids(Vec<i64>),
}

pub fn human_size(b: u64) -> String {
    let b = b as f64;
    if b >= 1e9 {
        // 小数点跟着界面语言（德文、法文、西班牙文用逗号）
        format!("{} GB", crate::i18n::decimal(b / 1e9, 2))
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} KB", b / 1e3)
    } else {
        format!("{b} B")
    }
}

pub struct Library {
    db: Mutex<Connection>,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).with_context(|| crate::tr!("打开素材库失败：{path}", path = path.display()))?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch(SCHEMA)?;
        Self::migrate(&db)?;
        Ok(Library { db: Mutex::new(db) })
    }

    pub fn memory() -> Result<Self> {
        let db = Connection::open_in_memory()?;
        db.execute_batch(SCHEMA)?;
        Self::migrate(&db)?;
        Ok(Library { db: Mutex::new(db) })
    }

    /// 后来加的列：没有就补上（9 月的第一版建的素材库、新建的素材库都走这里）。
    /// 新列都带默认值：退回旧版 Captae 时它照样能读写（旧版的 INSERT 写明了列名，读的时候按列名取，多出来的列不碍事）
    fn migrate(db: &Connection) -> Result<()> {
        let has = |col: &str| -> Result<bool> {
            let mut st = db.prepare("SELECT 1 FROM pragma_table_info('assets') WHERE name = ?1")?;
            Ok(st.exists([col])?)
        };
        // 两个进程同时打开、同时加列时，后加的那个会报「列已经有了」，再看一眼有就行
        let add = |table: &str, col: &str, ddl: &str| -> Result<()> {
            let has = |c: &str| -> Result<bool> {
                let mut st = db.prepare(&format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1"))?;
                Ok(st.exists([c])?)
            };
            if !has(col)? {
                if let Err(e) = db.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {ddl}")) {
                    if !has(col)? {
                        return Err(e).context(format!("ALTER TABLE {table} ADD COLUMN {col}"));
                    }
                }
            }
            Ok(())
        };
        // live：录直播录下来的（1）
        if !has("live")? {
            if let Err(e) = db.execute_batch("ALTER TABLE assets ADD COLUMN live INTEGER NOT NULL DEFAULT 0") {
                if !has("live")? {
                    return Err(e).context(crate::tr!("素材库补上 live 列失败"));
                }
            }
        }
        // 分类（2026-11-28）：不加外键，删分类时自己把素材放回未分类（delete_category）
        add("assets", "category_id", "category_id INTEGER")?;
        add("assets", "category_by", "category_by TEXT NOT NULL DEFAULT ''")?;
        add("tags", "by", "by TEXT NOT NULL DEFAULT 'user'")?;
        db.execute_batch("CREATE INDEX IF NOT EXISTS assets_category ON assets(category_id)")?;
        Ok(())
    }

    /// 全文检索的「标签」那一栏也放分类名和 AI 看封面写的一句描述（asset_cache 的 ai_caption）：搜「设计参考」「日程」都找得到
    fn reindex(db: &Connection, id: i64) -> Result<()> {
        db.execute("DELETE FROM assets_fts WHERE rowid = ?1", [id])?;
        db.execute(
            "INSERT INTO assets_fts(rowid, title, description, tags, site, transcript)
             SELECT a.id, a.title, a.description,
                    trim(COALESCE((SELECT group_concat(tag, ' ') FROM tags WHERE asset_id = a.id), '') || ' ' ||
                         COALESCE((SELECT name FROM categories WHERE id = a.category_id), '') || ' ' ||
                         COALESCE((SELECT value FROM asset_cache WHERE asset_id = a.id AND key = 'ai_caption'), '')),
                    a.site, a.transcript
             FROM assets a WHERE a.id = ?1",
            [id],
        )?;
        Ok(())
    }

    pub fn add(&self, a: &NewAsset) -> Result<i64> {
        let db = self.db.lock().unwrap();
        let now = crate::util::now_ms();
        let path = a.path.display().to_string();
        // 同一路径再次下载：更新元数据，保留原 id 和标签
        let existing: Option<i64> = db.query_row("SELECT id FROM assets WHERE path = ?1", [&path], |r| r.get(0)).optional()?;
        let id = if let Some(id) = existing {
            db.execute(
                "UPDATE assets SET kind=?2, title=?3, description=?4, source_url=?5, page_url=?6, site=?7, uploader=?8, format=?9,
                 width=?10, height=?11, duration=?12, size=?13, thumb=?14, created_at=?15, live=?16 WHERE id=?1",
                params![id, a.kind, a.title, a.description, a.source_url, a.page_url, a.site, a.uploader, a.format, a.width, a.height, a.duration, a.size as i64, a.thumb.as_ref().map(|p| p.display().to_string()), now, a.live],
            )?;
            id
        } else {
            db.execute(
                "INSERT INTO assets(kind,title,description,source_url,page_url,site,uploader,path,format,width,height,duration,size,thumb,parent_id,created_at,live)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
                params![a.kind, a.title, a.description, a.source_url, a.page_url, a.site, a.uploader, path, a.format, a.width, a.height, a.duration, a.size as i64, a.thumb.as_ref().map(|p| p.display().to_string()), a.parent_id, now, a.live],
            )?;
            db.last_insert_rowid()
        };
        for t in &a.tags {
            db.execute("INSERT OR IGNORE INTO tags(asset_id, tag) VALUES (?1, ?2)", params![id, t.trim()])?;
        }
        Self::reindex(&db, id)?;
        Ok(id)
    }

    pub fn set_transcript(&self, id: i64, text: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("UPDATE assets SET transcript = ?2 WHERE id = ?1", params![id, text])?;
        Self::reindex(&db, id)
    }

    pub fn add_tags(&self, id: i64, tags: &[String]) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        for t in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
            // AI 加过的同一个标签，用户再加一次就算用户的（清 AI 标签时留着）
            db.execute("INSERT INTO tags(asset_id, tag, by) VALUES (?1, ?2, 'user') ON CONFLICT(asset_id, tag) DO UPDATE SET by = 'user'", params![id, t])?;
        }
        Self::reindex(&db, id)?;
        Self::tags_of(&db, id)
    }

    pub fn remove_tag(&self, id: i64, tag: &str) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM tags WHERE asset_id = ?1 AND tag = ?2", params![id, tag])?;
        Self::reindex(&db, id)?;
        Self::tags_of(&db, id)
    }

    pub fn set_path(&self, id: i64, p: &Path) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("UPDATE assets SET path = ?2 WHERE id = ?1", params![id, p.display().to_string()])?;
        Ok(())
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM assets WHERE id = ?1", [id])?;
        db.execute("DELETE FROM assets_fts WHERE rowid = ?1", [id])?;
        Ok(())
    }

    fn ai_tags_of(db: &Connection, id: i64) -> Result<Vec<String>> {
        let mut st = db.prepare_cached("SELECT tag FROM tags WHERE asset_id = ?1 AND by = 'ai' ORDER BY rowid")?;
        let v = st.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(v)
    }

    fn tags_of(db: &Connection, id: i64) -> Result<Vec<String>> {
        let mut st = db.prepare_cached("SELECT tag FROM tags WHERE asset_id = ?1 ORDER BY rowid")?;
        let v = st.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(v)
    }

    fn row(db: &Connection, r: &rusqlite::Row) -> rusqlite::Result<Asset> {
        let id: i64 = r.get("id")?;
        let path: String = r.get("path")?;
        let path = PathBuf::from(path);
        let w: Option<u32> = r.get("width")?;
        let h: Option<u32> = r.get("height")?;
        let dur: Option<f64> = r.get("duration")?;
        let size: i64 = r.get("size")?;
        let created: i64 = r.get("created_at")?;
        let transcript: String = r.get("transcript")?;
        let day = (crate::util::now_ms() - created).max(0) / 86_400_000;
        Ok(Asset {
            id,
            kind: r.get("kind")?,
            title: r.get("title")?,
            desc: r.get("description")?,
            url: r.get("source_url")?,
            page: r.get("page_url")?,
            src: r.get("site")?,
            uploader: r.get("uploader")?,
            fmt: r.get("format")?,
            width: w,
            height: h,
            duration: dur,
            bytes: size as u64,
            thumb: r.get::<_, Option<String>>("thumb")?.map(PathBuf::from),
            tags: Self::tags_of(db, id).unwrap_or_default(),
            created_at: created,
            parent_id: r.get("parent_id")?,
            has_transcript: !transcript.is_empty(),
            live: r.get::<_, Option<i64>>("live").ok().flatten().unwrap_or(0) != 0,
            category_id: r.get::<_, Option<i64>>("category_id").ok().flatten(),
            category_by: r.get::<_, Option<String>>("category_by").ok().flatten().unwrap_or_default(),
            ai_tags: Self::ai_tags_of(db, id).unwrap_or_default(),
            res: match (w, h) {
                (Some(w), Some(h)) => format!("{w}×{h}"),
                _ => "—".into(),
            },
            dur: dur.map(crate::util::clock).unwrap_or_default(),
            size: human_size(size as u64),
            when: crate::util::when(created),
            day,
            exists: path.exists(),
            path,
        })
    }

    pub fn get(&self, id: i64) -> Result<Option<Asset>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare_cached("SELECT * FROM assets WHERE id = ?1")?;
        Ok(st.query_row([id], |r| Self::row(&db, r)).optional()?)
    }

    pub fn by_path(&self, p: &Path) -> Result<Option<Asset>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare_cached("SELECT * FROM assets WHERE path = ?1")?;
        Ok(st.query_row([p.display().to_string()], |r| Self::row(&db, r)).optional()?)
    }

    pub fn transcript(&self, id: i64) -> Result<String> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT transcript FROM assets WHERE id = ?1", [id], |r| r.get(0))?)
    }

    pub fn query(&self, q: &Query) -> Result<(Vec<Asset>, u64)> {
        let db = self.db.lock().unwrap();
        let mut sql = String::from(" FROM assets a WHERE 1=1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        if let Some(text) = q.q.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            // trigram 分词要求至少 3 个字符；短词退回 LIKE
            if text.chars().count() >= 3 {
                sql.push_str(" AND a.id IN (SELECT rowid FROM assets_fts WHERE assets_fts MATCH ?)");
                args.push(Box::new(format!("\"{}\"", text.replace('"', "\"\""))));
            } else {
                sql.push_str(
                    " AND (a.title LIKE ? OR a.description LIKE ? OR a.site LIKE ? OR a.id IN (SELECT asset_id FROM tags WHERE tag LIKE ?)
                      OR a.category_id IN (SELECT id FROM categories WHERE name LIKE ?) OR a.id IN (SELECT asset_id FROM asset_cache WHERE key = 'ai_caption' AND value LIKE ?))",
                );
                let like = format!("%{text}%");
                for _ in 0..6 {
                    args.push(Box::new(like.clone()));
                }
            }
        }
        if let Some(k) = &q.kind {
            sql.push_str(" AND a.kind = ?");
            args.push(Box::new(k.clone()));
        }
        if let Some(s) = &q.site {
            sql.push_str(" AND lower(a.site) = lower(?)");
            args.push(Box::new(s.clone()));
        }
        if let Some(f) = &q.format {
            sql.push_str(" AND upper(a.format) = upper(?)");
            args.push(Box::new(f.clone()));
        }
        if let Some(d) = q.days {
            sql.push_str(" AND a.created_at >= ?");
            args.push(Box::new(crate::util::now_ms() - d * 86_400_000));
        }
        if let Some(s) = &q.since {
            if let Ok(d) = time::Date::parse(s, &time::macros::format_description!("[year]-[month]-[day]")) {
                let ts = d.midnight().assume_utc().unix_timestamp() * 1000;
                sql.push_str(" AND a.created_at >= ?");
                args.push(Box::new(ts));
            }
        }
        for t in &q.tags {
            sql.push_str(" AND a.id IN (SELECT asset_id FROM tags WHERE tag = ?)");
            args.push(Box::new(t.clone()));
        }
        match q.cat {
            Some(0) => sql.push_str(" AND a.category_id IS NULL"),
            Some(c) => {
                sql.push_str(" AND a.category_id = ?");
                args.push(Box::new(c));
            }
            None => {}
        }
        if let Some(name) = q.category.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            sql.push_str(" AND a.category_id IN (SELECT id FROM categories WHERE lower(name) = lower(?))");
            args.push(Box::new(name.to_string()));
        }
        let total: u64 = {
            let mut st = db.prepare(&format!("SELECT count(*){sql}"))?;
            st.query_row(rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())), |r| r.get::<_, i64>(0))? as u64
        };
        sql.push_str(" ORDER BY a.created_at DESC LIMIT ? OFFSET ?");
        args.push(Box::new(q.limit.unwrap_or(200).min(1000)));
        args.push(Box::new(q.offset.unwrap_or(0)));
        let mut st = db.prepare(&format!("SELECT a.*{sql}"))?;
        let rows = st.query_map(rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())), |r| Self::row(&db, r))?;
        Ok((rows.collect::<rusqlite::Result<Vec<_>>>()?, total))
    }

    /// 筛选条上的「来源」按钮
    pub fn sites(&self) -> Result<Vec<(String, u64)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT site, count(*) FROM assets WHERE site != '' GROUP BY site ORDER BY count(*) DESC")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// 筛选条上的「标签」，按使用次数排
    pub fn tags(&self) -> Result<Vec<(String, u64)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT tag, count(*) FROM tags GROUP BY tag ORDER BY count(*) DESC, tag")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// 读缓存：返回 (内容, 写入时间 ms)
    pub fn cache_get(&self, id: i64, key: &str) -> Result<Option<(String, i64)>> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT value, at FROM asset_cache WHERE asset_id = ?1 AND key = ?2", params![id, key], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
    }

    pub fn cache_set(&self, id: i64, key: &str, value: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("INSERT OR REPLACE INTO asset_cache(asset_id, key, value, at) VALUES (?1, ?2, ?3, ?4)", params![id, key, value, crate::util::now_ms()])?;
        Ok(())
    }

    pub fn count(&self) -> Result<u64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT count(*) FROM assets", [], |r| r.get::<_, i64>(0))? as u64)
    }

    // ---- 分类 ----

    /// 全部分类（按排序），各带几条素材；第二个值是未分类的条数
    pub fn categories(&self) -> Result<(Vec<Category>, u64)> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare_cached(
            "SELECT c.id, c.name, c.description, c.sort, c.by, (SELECT count(*) FROM assets a WHERE a.category_id = c.id) FROM categories c ORDER BY c.sort, c.id",
        )?;
        let v = st
            .query_map([], |r| Ok(Category { id: r.get(0)?, name: r.get(1)?, desc: r.get(2)?, sort: r.get(3)?, by: r.get(4)?, count: r.get::<_, i64>(5)? as u64 }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let none: i64 = db.query_row("SELECT count(*) FROM assets WHERE category_id IS NULL", [], |r| r.get(0))?;
        Ok((v, none as u64))
    }

    /// 建一个分类；同名的（不分大小写）已经有了就用它。返回 id
    pub fn add_category(&self, name: &str, desc: &str, by: &str) -> Result<i64> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), crate::tr!("分类名不能是空的"));
        let db = self.db.lock().unwrap();
        if let Some(id) = db.query_row("SELECT id FROM categories WHERE lower(name) = lower(?1)", [name], |r| r.get::<_, i64>(0)).optional()? {
            return Ok(id);
        }
        let sort: i64 = db.query_row("SELECT COALESCE(max(sort), 0) + 1 FROM categories", [], |r| r.get(0))?;
        db.execute(
            "INSERT INTO categories(name, description, sort, by, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, desc.trim(), sort, by, crate::util::now_ms()],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// 改名、改说明（None 不改）。用户改过的分类算用户的，以后重新整理也留着
    pub fn update_category(&self, id: i64, name: Option<&str>, desc: Option<&str>, by: Option<&str>) -> Result<()> {
        let db = self.db.lock().unwrap();
        if let Some(n) = name.map(str::trim) {
            anyhow::ensure!(!n.is_empty(), crate::tr!("分类名不能是空的"));
            let clash: Option<i64> = db.query_row("SELECT id FROM categories WHERE lower(name) = lower(?1) AND id != ?2", params![n, id], |r| r.get(0)).optional()?;
            anyhow::ensure!(clash.is_none(), crate::tr!("已经有叫「{name}」的分类了", name = n));
            db.execute("UPDATE categories SET name = ?2 WHERE id = ?1", params![id, n])?;
        }
        if let Some(d) = desc {
            db.execute("UPDATE categories SET description = ?2 WHERE id = ?1", params![id, d.trim()])?;
        }
        if let Some(b) = by {
            db.execute("UPDATE categories SET by = ?2 WHERE id = ?1", params![id, b])?;
        }
        if name.is_some() {
            Self::reindex_category(&db, id)?;
        }
        Ok(())
    }

    fn reindex_category(db: &Connection, id: i64) -> Result<()> {
        let ids: Vec<i64> = db.prepare("SELECT id FROM assets WHERE category_id = ?1")?.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for a in ids {
            Self::reindex(db, a)?;
        }
        Ok(())
    }

    /// 删掉分类，里面的素材放回未分类（谁放的照旧记着：用户亲手放的不会被自动整理拿走）
    pub fn delete_category(&self, id: i64) -> Result<()> {
        let db = self.db.lock().unwrap();
        let ids: Vec<i64> = db.prepare("SELECT id FROM assets WHERE category_id = ?1")?.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        db.execute("UPDATE assets SET category_id = NULL WHERE category_id = ?1", [id])?;
        db.execute("DELETE FROM categories WHERE id = ?1", [id])?;
        for a in ids {
            Self::reindex(&db, a)?;
        }
        Ok(())
    }

    /// 把 from 合并进 into：素材挪过去，from 删掉
    pub fn merge_category(&self, from: i64, into: i64) -> Result<()> {
        anyhow::ensure!(from != into, "same category");
        let db = self.db.lock().unwrap();
        db.execute("UPDATE assets SET category_id = ?2 WHERE category_id = ?1", params![from, into])?;
        db.execute("DELETE FROM categories WHERE id = ?1", [from])?;
        Self::reindex_category(&db, into)?;
        Ok(())
    }

    /// 按给的顺序排（没列到的排在后面）
    pub fn reorder_categories(&self, ids: &[i64]) -> Result<()> {
        let db = self.db.lock().unwrap();
        for (i, id) in ids.iter().enumerate() {
            db.execute("UPDATE categories SET sort = ?2 WHERE id = ?1", params![id, i as i64 + 1])?;
        }
        Ok(())
    }

    /// 放进分类（None = 未分类）。by：user 用户亲手放的 / ai
    pub fn set_category(&self, ids: &[i64], cat: Option<i64>, by: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        for id in ids {
            db.execute("UPDATE assets SET category_id = ?2, category_by = ?3 WHERE id = ?1", params![id, cat, by])?;
            Self::reindex(&db, *id)?;
        }
        Ok(())
    }

    /// 换掉这条素材上 AI 加的标签（用户加的不动；和用户的同名的不再加）
    pub fn set_ai_tags(&self, id: i64, tags: &[String]) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM tags WHERE asset_id = ?1 AND by = 'ai'", [id])?;
        for t in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
            db.execute("INSERT OR IGNORE INTO tags(asset_id, tag, by) VALUES (?1, ?2, 'ai')", params![id, t])?;
        }
        Self::reindex(&db, id)
    }

    /// 清掉所有 AI 加的标签，返回清了几个
    pub fn clear_ai_tags(&self) -> Result<u64> {
        let db = self.db.lock().unwrap();
        let ids: Vec<i64> = db.prepare("SELECT DISTINCT asset_id FROM tags WHERE by = 'ai'")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let n = db.execute("DELETE FROM tags WHERE by = 'ai'", [])?;
        for id in ids {
            Self::reindex(&db, id)?;
        }
        Ok(n as u64)
    }

    /// 整理时要看的素材（新的在前）
    pub fn pick(&self, p: &Pick) -> Result<Vec<Asset>> {
        let db = self.db.lock().unwrap();
        let (cond, ids): (&str, Vec<i64>) = match p {
            Pick::NotByUser => ("category_by != 'user'", vec![]),
            Pick::Uncategorized => ("category_id IS NULL AND category_by != 'user'", vec![]),
            Pick::Never => ("category_by = ''", vec![]),
            Pick::Ids(v) => ("1=1", v.clone()),
        };
        let mut out = vec![];
        if let Pick::Ids(_) = p {
            let mut st = db.prepare_cached("SELECT * FROM assets WHERE id = ?1")?;
            for id in ids {
                if let Some(a) = st.query_row([id], |r| Self::row(&db, r)).optional()? {
                    out.push(a);
                }
            }
            return Ok(out);
        }
        let mut st = db.prepare(&format!("SELECT * FROM assets WHERE {cond} ORDER BY created_at DESC"))?;
        let v = st.query_map([], |r| Self::row(&db, r))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// 用户亲手放进分类的最近几条：带进给模型的提示词当例子
    pub fn user_examples(&self, limit: u32) -> Result<Vec<(Asset, String)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare(
            "SELECT a.*, c.name AS cat_name FROM assets a JOIN categories c ON c.id = a.category_id WHERE a.category_by = 'user' ORDER BY a.created_at DESC LIMIT ?1",
        )?;
        let v = st.query_map([limit], |r| Ok((Self::row(&db, r)?, r.get::<_, String>("cat_name")?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// 用户亲手定过分类的素材：(素材 id, 分类 id，放进未分类的是 None)
    pub fn user_filed(&self) -> Result<Vec<(i64, Option<i64>)>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare_cached("SELECT id, category_id FROM assets WHERE category_by = 'user'")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// 写缓存并更新全文检索（AI 看封面写的描述也要能搜到）
    pub fn cache_set_indexed(&self, id: i64, key: &str, value: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("INSERT OR REPLACE INTO asset_cache(asset_id, key, value, at) VALUES (?1, ?2, ?3, ?4)", params![id, key, value, crate::util::now_ms()])?;
        Self::reindex(&db, id)
    }

    // ---- Agent 调用记录 ----
    pub fn call_start(&self, client: &str, tool: &str, args: &serde_json::Value) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO agent_calls(at, client, tool, args, status) VALUES (?1, ?2, ?3, ?4, 'run')",
            params![crate::util::now_ms(), client, tool, args.to_string()],
        )?;
        let id = db.last_insert_rowid();
        // 只保留最近 500 条
        db.execute("DELETE FROM agent_calls WHERE id <= ?1 - 500", [id])?;
        Ok(id)
    }

    /// 上次退出时还没做完的调用，标成出错，不然日志里会一直转圈。
    pub fn interrupt_running_calls(&self) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE agent_calls SET status = 'err', result = ?1 WHERE status = 'run'",
            params![serde_json::json!({"error": "INTERRUPTED", "message": crate::tr!("Captae 关闭时这次调用还没做完")}).to_string()],
        )?;
        Ok(())
    }

    pub fn call_end(&self, id: i64, ok: bool, result: &serde_json::Value, ms: i64) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("UPDATE agent_calls SET status = ?2, result = ?3, ms = ?4 WHERE id = ?1", params![id, if ok { "ok" } else { "err" }, result.to_string(), ms])?;
        Ok(())
    }

    pub fn calls(&self, limit: u32) -> Result<Vec<crate::agent::CallRecord>> {
        let db = self.db.lock().unwrap();
        let mut st = db.prepare("SELECT id, at, client, tool, args, status, result, ms FROM agent_calls ORDER BY id DESC LIMIT ?1")?;
        let v = st
            .query_map([limit], |r| {
                Ok(crate::agent::CallRecord {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    who: r.get(2)?,
                    tool: r.get(3)?,
                    args: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
                    st: r.get(5)?,
                    res: r.get::<_, Option<String>>(6)?.and_then(|s| serde_json::from_str(&s).ok()),
                    ms: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    // ---- 浏览器插件：下载前查一下是不是已经下过 ----
    /// 按下载链接找素材；`page` 为真时，来源页面等于这个链接的视频也算。
    /// 原始下载优先于转换/剪辑的产物，文件已经不在的跳过。
    /// 录直播录下来的不算（`live`）：直播频道的链接每次录的是不同的内容，不能说「下载过了」
    pub fn find_by_url(&self, url: &str, page: bool) -> Result<Option<Asset>> {
        let url = url.split('#').next().unwrap_or(url).trim();
        if url.is_empty() {
            return Ok(None);
        }
        let db = self.db.lock().unwrap();
        let mut st = db.prepare_cached(
            "SELECT * FROM assets WHERE (source_url = ?1 OR (?2 AND page_url = ?1 AND kind = 'video')) AND live = 0
             ORDER BY parent_id IS NOT NULL, created_at DESC LIMIT 20",
        )?;
        let rows = st.query_map(params![url, page], |r| Self::row(&db, r))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows.into_iter().find(|a| a.exists))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lib() -> Library {
        let l = Library::memory().unwrap();
        let a = |title: &str, kind: &str, site: &str, path: &str, tags: &[&str]| NewAsset {
            kind: kind.into(),
            title: title.into(),
            description: format!("{title} description"),
            site: site.into(),
            path: path.into(),
            format: "MP4".into(),
            size: 612_000_000,
            duration: Some(888.0),
            width: Some(1920),
            height: Some(818),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        l.add(&a("Sintel · Open Movie", "video", "YouTube", "/m/Sintel.mp4", &["短片", "奇幻"])).unwrap();
        l.add(&a("Tears of Steel", "video", "blender.org", "/m/tos.mp4", &["科幻"])).unwrap();
        l.add(&a("Sintel 原声", "audio", "YouTube", "/m/Sintel.mp3", &["配乐"])).unwrap();
        l
    }
    #[test]
    fn filters() {
        let l = lib();
        assert_eq!(l.query(&Query::default()).unwrap().1, 3);
        assert_eq!(l.query(&Query { kind: Some("audio".into()), ..Default::default() }).unwrap().1, 1);
        assert_eq!(l.query(&Query { site: Some("youtube".into()), ..Default::default() }).unwrap().1, 2);
        assert_eq!(l.query(&Query { tags: vec!["科幻".into()], ..Default::default() }).unwrap().0[0].title, "Tears of Steel");
        assert_eq!(l.query(&Query { q: Some("sintel".into()), ..Default::default() }).unwrap().1, 2);
        // 中文两个字走 LIKE，也能搜标签
        assert_eq!(l.query(&Query { q: Some("奇幻".into()), ..Default::default() }).unwrap().1, 1);
    }
    #[test]
    fn tags_and_fts() {
        let l = lib();
        let (v, _) = l.query(&Query { q: Some("Tears".into()), ..Default::default() }).unwrap();
        let id = v[0].id;
        l.add_tags(id, &["参考片段".into()]).unwrap();
        assert_eq!(l.query(&Query { q: Some("参考片段".into()), ..Default::default() }).unwrap().1, 1);
        l.set_transcript(id, "we need to rebuild the robot arm").unwrap();
        assert_eq!(l.query(&Query { q: Some("robot arm".into()), ..Default::default() }).unwrap().0[0].id, id);
        l.remove_tag(id, "参考片段").unwrap();
        assert_eq!(l.query(&Query { q: Some("参考片段".into()), ..Default::default() }).unwrap().1, 0);
        let a = l.get(id).unwrap().unwrap();
        assert_eq!((a.res.as_str(), a.dur.as_str(), a.size.as_str()), ("1920×818", "14:48", "612 MB"));
    }
    #[test]
    fn tag_counts_and_paging() {
        let l = lib();
        let t = l.tags().unwrap();
        assert_eq!(t.len(), 4);
        assert!(t.iter().all(|(_, n)| *n == 1));
        let id = l.by_path(Path::new("/m/tos.mp4")).unwrap().unwrap().id;
        l.add_tags(id, &["短片".into()]).unwrap();
        assert_eq!(l.tags().unwrap()[0], ("短片".to_string(), 2));
        // 分页：总数不受 limit / offset 影响，两页拼起来就是全部
        let q = |offset| Query { limit: Some(2), offset: Some(offset), ..Default::default() };
        let (a, total) = l.query(&q(0)).unwrap();
        let (b, total2) = l.query(&q(2)).unwrap();
        assert_eq!((a.len(), b.len(), total, total2), (2, 1, 3, 3));
        assert!(a.iter().all(|x| x.id != b[0].id));
        // 关键词 + 标签 + 分页一起用
        let (v, n) = l.query(&Query { q: Some("sintel".into()), tags: vec!["短片".into()], limit: Some(1), ..Default::default() }).unwrap();
        assert_eq!((v.len(), n), (1, 1));
    }
    #[test]
    fn cache_and_remove() {
        let l = lib();
        let id = l.by_path(Path::new("/m/tos.mp4")).unwrap().unwrap().id;
        assert!(l.cache_get(id, "subs").unwrap().is_none());
        l.cache_set(id, "subs", "[1]").unwrap();
        l.cache_set(id, "subs", "[2]").unwrap();
        assert_eq!(l.cache_get(id, "subs").unwrap().unwrap().0, "[2]");
        l.remove(id).unwrap();
        assert!(l.get(id).unwrap().is_none());
        assert!(l.cache_get(id, "subs").unwrap().is_none());
        assert_eq!(l.query(&Query { q: Some("Tears".into()), ..Default::default() }).unwrap().1, 0);
        assert_eq!(l.tags().unwrap().iter().find(|(t, _)| t == "科幻"), None);
    }
    #[test]
    fn readd_keeps_id() {
        let l = lib();
        let id = l.by_path(Path::new("/m/tos.mp4")).unwrap().unwrap().id;
        let id2 = l.add(&NewAsset { kind: "video".into(), title: "ToS again".into(), path: "/m/tos.mp4".into(), ..Default::default() }).unwrap();
        assert_eq!(id, id2);
        assert_eq!(l.get(id).unwrap().unwrap().tags, vec!["科幻"]);
    }

    /// 最后一轮第 2 项：录直播录下来的不算「下载过了」（直播频道的链接每次录的是不同的内容），普通视频照旧
    #[test]
    fn live_recordings_are_not_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let f = |n: &str| {
            let p = dir.path().join(n);
            std::fs::write(&p, b"x").unwrap();
            p
        };
        let l = Library::memory().unwrap();
        // 同一个直播频道录过一次（整页交给 yt-dlp 的：来源链接和来源页面都是频道地址）
        let ch = "https://live.example.com/channel/1";
        let rec = l.add(&NewAsset { kind: "video".into(), title: "直播 2026-11-24 08_00".into(), source_url: ch.into(), page_url: ch.into(), path: f("rec.mp4"), live: true, ..Default::default() }).unwrap();
        assert!(l.get(rec).unwrap().unwrap().live);
        assert!(l.find_by_url(ch, false).unwrap().is_none() && l.find_by_url(ch, true).unwrap().is_none(), "录过的直播链接不算下载过了");
        // 直播录像转换出来的（带着同一个来源链接）也不算
        l.add(&NewAsset { kind: "audio".into(), title: "直播 mp3".into(), source_url: ch.into(), path: f("rec.mp3"), parent_id: Some(rec), live: true, ..Default::default() }).unwrap();
        assert!(l.find_by_url(ch, true).unwrap().is_none());
        // 本机直播 HLS 录的：插件按 M3U8 地址查
        let m3u8 = "http://127.0.0.1:18791/hls/live.m3u8";
        l.add(&NewAsset { kind: "video".into(), title: "live".into(), source_url: m3u8.into(), page_url: "http://127.0.0.1:18791/live.html".into(), path: f("live.mp4"), live: true, ..Default::default() }).unwrap();
        assert!(l.find_by_url(m3u8, false).unwrap().is_none() && l.find_by_url("http://127.0.0.1:18791/live.html", true).unwrap().is_none());
        // 普通视频照旧
        let vod = "https://video.example.com/watch/7";
        let id = l.add(&NewAsset { kind: "video".into(), title: "点播".into(), source_url: vod.into(), page_url: vod.into(), path: f("vod.mp4"), ..Default::default() }).unwrap();
        assert_eq!(l.find_by_url(vod, true).unwrap().map(|a| a.id), Some(id));
        assert!(!l.get(id).unwrap().unwrap().live);
        // 同一个链接既录过直播、又下过普通视频（直播结束后的回放）：按下过回放算
        let vod2 = l.add(&NewAsset { kind: "video".into(), title: "回放".into(), source_url: ch.into(), page_url: ch.into(), path: f("replay.mp4"), ..Default::default() }).unwrap();
        assert_eq!(l.find_by_url(ch, true).unwrap().map(|a| a.id), Some(vod2));
        // 同一个文件重新入库时跟着改（录像被当成普通下载重新入库就不再是直播）
        l.add(&NewAsset { kind: "video".into(), title: "live".into(), source_url: m3u8.into(), path: dir.path().join("live.mp4"), ..Default::default() }).unwrap();
        assert!(l.find_by_url(m3u8, false).unwrap().is_some());
    }

    /// 分类：建、改名、合并、删；素材放进去、按分类查；用户亲手放的记着；分类名和 AI 描述能搜到
    #[test]
    fn categories_basic() {
        let l = lib();
        let all = l.query(&Query::default()).unwrap().0;
        let (a, b, c) = (all[0].id, all[1].id, all[2].id);
        let film = l.add_category("影视", "电影和短片", "ai").unwrap();
        let music = l.add_category("音乐", "", "ai").unwrap();
        assert_eq!(l.add_category(" 影视 ", "", "user").unwrap(), film, "同名的用原来的");
        l.set_category(&[a, b], Some(film), "ai").unwrap();
        l.set_category(&[c], Some(music), "user").unwrap();
        let (cats, none) = l.categories().unwrap();
        assert_eq!((cats.len(), cats[0].count, cats[1].count, none), (2, 2, 1, 0));
        assert_eq!(l.query(&Query { cat: Some(film), ..Default::default() }).unwrap().1, 2);
        assert_eq!(l.query(&Query { category: Some("音乐".into()), ..Default::default() }).unwrap().1, 1);
        assert_eq!(l.get(c).unwrap().unwrap().category_by, "user");
        // 分类名进了全文检索；改名以后跟着变
        assert_eq!(l.query(&Query { q: Some("影视".into()), ..Default::default() }).unwrap().1, 2);
        l.update_category(film, Some("电影短片"), None, Some("user")).unwrap();
        assert_eq!(l.query(&Query { q: Some("电影短片".into()), ..Default::default() }).unwrap().1, 2);
        assert!(l.update_category(film, Some("音乐"), None, None).is_err(), "不能和别的分类重名");
        // 用户亲手放的不在「要整理的」里
        assert_eq!(l.pick(&Pick::NotByUser).unwrap().len(), 2);
        // 合并：素材挪过去
        l.merge_category(music, film).unwrap();
        let (cats, _) = l.categories().unwrap();
        assert_eq!((cats.len(), cats[0].count), (1, 3));
        // 删分类：素材回到未分类，谁放的照旧
        l.delete_category(film).unwrap();
        let (cats, none) = l.categories().unwrap();
        assert_eq!((cats.len(), none), (0, 3));
        assert_eq!(l.query(&Query { cat: Some(0), ..Default::default() }).unwrap().1, 3);
        assert_eq!(l.get(c).unwrap().unwrap().category_by, "user");
        assert_eq!(l.pick(&Pick::Uncategorized).unwrap().len(), 2);
        // AI 看封面写的描述也能搜到
        l.cache_set_indexed(a, "ai_caption", "日程管理软件的界面演示").unwrap();
        assert_eq!(l.query(&Query { q: Some("日程管理".into()), ..Default::default() }).unwrap().0[0].id, a);
    }

    /// AI 加的标签单独记着：换、清都只动 AI 的；用户再加一次同名的就算用户的
    #[test]
    fn ai_tags_are_separate() {
        let l = lib();
        let id = l.by_path(Path::new("/m/tos.mp4")).unwrap().unwrap().id;
        l.set_ai_tags(id, &["机器人".into(), "科幻".into(), "阿姆斯特丹".into()]).unwrap();
        let a = l.get(id).unwrap().unwrap();
        assert_eq!(a.tags, vec!["科幻", "机器人", "阿姆斯特丹"], "和用户同名的不重复加");
        assert_eq!(a.ai_tags, vec!["机器人", "阿姆斯特丹"]);
        l.add_tags(id, &["机器人".into()]).unwrap();
        assert_eq!(l.get(id).unwrap().unwrap().ai_tags, vec!["阿姆斯特丹"]);
        assert_eq!(l.query(&Query { q: Some("阿姆斯特丹".into()), ..Default::default() }).unwrap().1, 1);
        assert_eq!(l.clear_ai_tags().unwrap(), 1);
        assert_eq!(l.get(id).unwrap().unwrap().tags, vec!["科幻", "机器人"]);
    }

    /// 9 月的第一版建的素材库没有 live 列：打开时补上，原来的素材都不是直播、照常查得到；
    /// 补过列以后，旧版的写法（INSERT 写明列名、SELECT * 按列名取）照样能读写——退回旧版不会坏
    #[test]
    fn old_library_gets_live_column() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("captae.db");
        let file = dir.path().join("v.mp4");
        std::fs::write(&file, b"x").unwrap();
        {
            let c = Connection::open(&db).unwrap();
            c.execute_batch(SCHEMA).unwrap();
            assert!(!c.prepare("SELECT 1 FROM pragma_table_info('assets') WHERE name = 'live'").unwrap().exists([]).unwrap(), "第一版的表里没有 live");
            c.execute(
                "INSERT INTO assets(kind,title,description,source_url,page_url,site,uploader,path,format,width,height,duration,size,thumb,parent_id,created_at)
                 VALUES ('video','旧的','','https://a.com/v.mp4','https://a.com/watch/1','a.com','',?1,'MP4',640,360,10.0,1,NULL,NULL,1)",
                [file.display().to_string()],
            )
            .unwrap();
            c.execute("INSERT INTO tags(asset_id, tag) VALUES (1, '旧标签')", []).unwrap();
        }
        let l = Library::open(&db).unwrap();
        let a = l.find_by_url("https://a.com/watch/1", true).unwrap().unwrap();
        assert_eq!((a.title.as_str(), a.live, a.tags.clone()), ("旧的", false, vec!["旧标签".to_string()]));
        // 分类的列也加上了：旧素材是「还没整理过」，旧标签算用户的
        assert_eq!((a.category_id, a.category_by.as_str(), a.ai_tags.len()), (None, "", 0));
        l.add(&NewAsset { kind: "video".into(), title: "新录的".into(), source_url: "https://a.com/live".into(), path: dir.path().join("r.mp4"), live: true, ..Default::default() }).unwrap();
        drop(l);
        // 再打开一次：列已经有了，不再补
        let l = Library::open(&db).unwrap();
        assert_eq!(l.count().unwrap(), 2);
        drop(l);
        // 旧版的写法照样能用
        let c = Connection::open(&db).unwrap();
        c.execute(
            "INSERT INTO assets(kind,title,description,source_url,page_url,site,uploader,path,format,width,height,duration,size,thumb,parent_id,created_at)
             VALUES ('audio','旧版又下了一个','','https://b.com/a.mp3','','b.com','','/m/a.mp3','MP3',NULL,NULL,3.0,1,NULL,NULL,2)",
            [],
        )
        .unwrap();
        let titles: Vec<String> = c.prepare("SELECT * FROM assets ORDER BY id").unwrap().query_map([], |r| r.get("title")).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(titles, ["旧的", "新录的", "旧版又下了一个"]);
        let live: i64 = c.query_row("SELECT live FROM assets WHERE title = '旧版又下了一个'", [], |r| r.get(0)).unwrap();
        assert_eq!(live, 0, "旧版写进来的默认不是直播");
    }
}
