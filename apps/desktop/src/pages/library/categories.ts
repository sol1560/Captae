// 素材库的分类：放进分类、改名、合并、删掉、「用 AI 整理」要用的命令（后端 category_cmds.rs、core categorize.rs）。
// 界面组件只管画，命令和提示条都在这里。
import { lang, t } from "@/i18n";
import { errText, invoke } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { getState, loadCategories } from "@/lib/store";
import type { Category } from "@/lib/types";

/** 开始整理前：会发多少、发给谁（core categorize.rs 的 Estimate） */
export type Estimate = { hasModel: boolean; host: string; model: string; vision: string; items: number; covers: number; tokens: number; existing: number; userFiled: number };

/** 整理出来的一个分类（PCat） */
export type PCat = { key: string; name: string; desc: string; count: number; samples: { id: number; title: string; thumb?: string | null }[]; id?: number | null; fixed: boolean };
export type Proposal = { categories: PCat[]; total: number; none: number; usage: { prompt: number; completion: number } };
/** 用户在确认前对一个分类的改动 */
export type Edit = { key: string; name?: string; desc?: string; into?: string; drop?: boolean };

export const categoryName = (id: number | null | undefined, cats: Category[] = getState().categories) => (id == null ? null : (cats.find((c) => c.id === id)?.name ?? null));

/** 用户亲手把素材放进分类（cat 为空 = 未分类）：之后自动整理不再动它们 */
export async function setCategory(ids: number[], cat: number | null): Promise<boolean> {
  try {
    await invoke("asset_set_category", { ids, cat });
    loadCategories();
    return true;
  } catch (e) {
    notify(t("没有放进分类"), errText(e));
    return false;
  }
}

/** 多选后一起放进某个分类 */
export async function setCategoryMany(ids: number[], cat: number | null) {
  if (!(await setCategory(ids, cat))) return false;
  const name = categoryName(cat);
  notify(name ? t("{n}项放进了「{name}」", { n: ids.length, name }) : t("{n}项放回了未分类", { n: ids.length }));
  return true;
}

/** 新建（不带 id）或改名。返回分类 id，失败返回 null（重名之类，已经提示过） */
export async function saveCategory(name: string, id?: number, desc?: string): Promise<number | null> {
  try {
    const r = await invoke<number>("category_save", { id: id ?? null, name, desc: desc ?? null });
    await loadCategories();
    return r;
  } catch (e) {
    notify(id ? t("没有改名") : t("没有建好分类"), errText(e));
    return null;
  }
}

export async function deleteCategory(c: Category) {
  try {
    await invoke("category_delete", { id: c.id });
    await loadCategories();
    notify(t("删掉了「{name}」", { name: c.name }), c.count ? t("里面的{n}项放回了未分类", { n: c.count }) : undefined);
  } catch (e) {
    notify(t("没有删掉"), errText(e));
  }
}

export async function mergeCategory(from: Category, into: Category) {
  try {
    await invoke("category_merge", { from: from.id, into: into.id });
    await loadCategories();
    notify(t("「{from}」并进了「{into}」", { from: from.name, into: into.name }));
  } catch (e) {
    notify(t("没有合并"), errText(e));
  }
}

/** 「给未分类的归类」：后台跑，完了说一声 */
export async function fileRest() {
  notify(t("正在给未分类的素材归类"), t("好了会告诉你"));
  try {
    const r = await invoke<{ filed: number; none: number }>("categorize_rest");
    await loadCategories();
    // 两句独立的说明，各自翻译，中间按语言用「，」或「, 」隔开
    const note = [t("{n}项放进了分类", { n: r.filed }), r.none ? t("{n}项没把握，还在未分类", { n: r.none }) : ""].filter(Boolean).join(t("，§两句说明之间的分隔"));
    notify(t("归类好了"), note);
  } catch (e) {
    const msg = errText(e);
    if (msg !== "CANCELLED") notify(t("没有归类完"), msg.replace(/^[A-Z_]+: /, ""));
  }
}

export async function clearAiTags() {
  try {
    const n = await invoke<number>("ai_tags_clear");
    notify(n ? t("清掉了{n}个 AI 加的标签", { n }) : t("没有 AI 加的标签"));
  } catch (e) {
    notify(t("没有清掉"), errText(e));
  }
}

/** 「大约 1.2 万 token」：按界面语言写数字 */
export function tokenText(n: number): string {
  const rounded = n < 10000 ? Math.ceil(n / 100) * 100 : Math.ceil(n / 1000) * 1000;
  return rounded.toLocaleString(lang());
}
