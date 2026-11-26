// libytdlp.so：Android 完整版的 yt-dlp 启动器（MIT，Captae 自己写的）。
//
// captae-core 的 AndroidRunner 把 yt-dlp 当成一个程序来起（<原生库目录>/libytdlp.so <参数>），这个文件就是那个程序：
// 链接 python.org 官方 Android 包里的 libpython3.14.so，把参数原样交给 yt-dlp 的 zipapp。
//
// 为什么这样放：Android 10 起 App 不能 execve 自己可写目录里的文件，只有安装器解压到原生库目录的 lib*.so 能执行。
// 所以能执行、能加载的代码（这个启动器、libpython、扩展模块、ffmpeg、qjs）都放原生库目录；
// 可写目录里只放数据（标准库的 .py、yt-dlp 的 zipapp），由 libpython 读进来解释执行。
//
// 环境变量（App 启动时由 CaptaeEngine.prepare 用 Os.setenv 设好，子进程继承）：
//   CAPTAE_ENGINE_HOME  解压出来的数据目录：lib/python3.14/（标准库）、yt-dlp（随包的 zipapp）。没设时按包名推算
//                       /data/user/<用户>/<包名>/no_backup/captae-engine
//   CAPTAE_YTDLP        要跑的 yt-dlp zipapp（应用内更新装的新版、自检时用）；没设用 <HOME>/yt-dlp
//   TMPDIR              临时目录（yt-dlp 给 QuickJS 写脚本文件要用）；没设时用 <HOME>/tmp
//   SSL_CERT_FILE       根证书（PEM）。App 里由 CaptaeEngine.prepare 从系统证书库（含用户装的证书）导出。
//                       没设时（adb shell 里测试）把系统证书目录里的文件拼成 <HOME>/tmp/cacerts.pem 再用：
//                       Android 的证书目录按旧式哈希（subject_hash_old）命名，OpenSSL 3 当 SSL_CERT_DIR 用找不到证书。
//
// 扩展模块（_ssl、_sqlite3……）在原生库目录里叫 libpy_<模块名>.so，由 captae_boot.py 装的查找器加载，见那个文件。

#include <Python.h>

#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define PY_DIR "lib/python3.14"

static int is_file(const char *p) {
    struct stat st;
    return stat(p, &st) == 0 && S_ISREG(st.st_mode);
}

static int is_dir(const char *p) {
    struct stat st;
    return stat(p, &st) == 0 && S_ISDIR(st.st_mode);
}

static void die(const char *msg, const char *detail) {
    // 写成 yt-dlp 的错误格式，captae-core 的错误分类能认出是组件问题
    fprintf(stderr, "ERROR: Captae engine: %s%s%s\n", msg, detail ? ": " : "", detail ? detail : "");
    exit(127);
}

// 自己所在的目录（原生库目录）
static void self_dir(char *out, size_t n) {
    char buf[PATH_MAX];
    ssize_t len = readlink("/proc/self/exe", buf, sizeof(buf) - 1);
    if (len <= 0) die("cannot read /proc/self/exe", strerror(errno));
    buf[len] = 0;
    char *slash = strrchr(buf, '/');
    if (!slash) die("unexpected executable path", buf);
    *slash = 0;
    snprintf(out, n, "%s", buf);
}

// 没设 CAPTAE_ENGINE_HOME 时按原生库目录推算数据目录：
// 原生库目录形如 /data/app/~~随机==/<包名>-随机==/lib/arm64，包名里不会有「-」
static int guess_home(const char *libdir, char *out, size_t n) {
    const char *lib = strstr(libdir, "/lib/");
    if (!lib) return 0;
    const char *start = lib;
    while (start > libdir && start[-1] != '/') start--;
    const char *dash = memchr(start, '-', (size_t)(lib - start));
    size_t len = (size_t)((dash ? dash : lib) - start);
    if (len == 0 || len > 200) return 0;
    char pkg[256];
    memcpy(pkg, start, len);
    pkg[len] = 0;
    snprintf(out, n, "/data/user/%u/%s/no_backup/captae-engine", (unsigned)(getuid() / 100000), pkg);
    return 1;
}

// 把系统证书目录里的证书拼成一个 PEM 文件（每个文件是 PEM 加一段文字说明，OpenSSL 读 PEM 时跳过说明）。成功返回 1
static int build_cert_bundle(const char *out) {
    static const char *dirs[] = {"/apex/com.android.conscrypt/cacerts", "/system/etc/security/cacerts"};
    for (size_t i = 0; i < sizeof(dirs) / sizeof(dirs[0]); i++) {
        DIR *d = opendir(dirs[i]);
        if (!d) continue;
        char part[PATH_MAX];
        snprintf(part, sizeof(part), "%s.%d.part", out, (int)getpid());
        FILE *f = fopen(part, "w");
        if (!f) {
            closedir(d);
            return 0;
        }
        int n = 0;
        struct dirent *e;
        char p[PATH_MAX], buf[8192];
        while ((e = readdir(d)) != NULL) {
            if (e->d_name[0] == '.') continue;
            snprintf(p, sizeof(p), "%s/%s", dirs[i], e->d_name);
            FILE *c = fopen(p, "r");
            if (!c) continue;
            size_t k;
            while ((k = fread(buf, 1, sizeof(buf), c)) > 0) fwrite(buf, 1, k, f);
            fputc('\n', f);
            fclose(c);
            n++;
        }
        closedir(d);
        if (fclose(f) != 0 || n == 0 || rename(part, out) != 0) {
            unlink(part);
            continue;
        }
        return 1;
    }
    return 0;
}

static wchar_t *wide(const char *s) {
    wchar_t *w = Py_DecodeLocale(s, NULL);
    if (!w) die("cannot decode argument", s);
    return w;
}

static void check(PyStatus st) {
    if (PyStatus_Exception(st)) {
        if (PyStatus_IsExit(st)) exit(st.exitcode);
        die("python init failed", st.err_msg ? st.err_msg : "?");
    }
}

int main(int argc, char **argv) {
    char libdir[PATH_MAX], home[PATH_MAX], path[PATH_MAX];
    self_dir(libdir, sizeof(libdir));

    const char *h = getenv("CAPTAE_ENGINE_HOME");
    if (h && *h) {
        snprintf(home, sizeof(home), "%s", h);
    } else if (!guess_home(libdir, home, sizeof(home))) {
        die("CAPTAE_ENGINE_HOME is not set", NULL);
    }
    snprintf(path, sizeof(path), "%s/" PY_DIR "/os.py", home);
    if (!is_file(path)) die("Python standard library not extracted (call CaptaeEngine.prepare first)", path);

    char ytdlp[PATH_MAX];
    const char *y = getenv("CAPTAE_YTDLP");
    if (y && *y) {
        snprintf(ytdlp, sizeof(ytdlp), "%s", y);
    } else {
        snprintf(ytdlp, sizeof(ytdlp), "%s/yt-dlp", home);
    }
    if (!is_file(ytdlp)) die("yt-dlp not found", ytdlp);

    // 给子进程（ffmpeg、qjs）和 Python 自己用的环境
    setenv("CAPTAE_ENGINE_LIBDIR", libdir, 1);
    const char *tmp = getenv("TMPDIR");
    if (!tmp || !*tmp || !is_dir(tmp)) {
        snprintf(path, sizeof(path), "%s/tmp", home);
        mkdir(path, 0700);
        setenv("TMPDIR", path, 1);
    }
    const char *cert = getenv("SSL_CERT_FILE");
    if (!cert || !*cert || !is_file(cert)) {
        snprintf(path, sizeof(path), "%s/tmp", home);
        mkdir(path, 0700);
        snprintf(path, sizeof(path), "%s/tmp/cacerts.pem", home);
        if (is_file(path) || build_cert_bundle(path)) setenv("SSL_CERT_FILE", path, 1);
    }

    // UTF-8 模式：yt-dlp 的输出、文件名都按 UTF-8，不看设备语言
    PyPreConfig pre;
    PyPreConfig_InitIsolatedConfig(&pre);
    pre.utf8_mode = 1;
    check(Py_PreInitialize(&pre));

    PyConfig config;
    PyConfig_InitIsolatedConfig(&config);
    check(PyConfig_SetBytesString(&config, &config.home, home));
    check(PyConfig_SetBytesString(&config, &config.program_name, "yt-dlp"));
    // 不导入 site（没有 site-packages，省启动时间）；标准库目录可写，第一次运行时写 __pycache__，之后启动快
    config.site_import = 0;
    config.write_bytecode = 1;
    config.buffered_stdio = 0;
    // 要收 SIGINT：录直播时 captae-core 发 SIGINT 让 yt-dlp 正常收尾
    config.install_signal_handlers = 1;
    config.parse_argv = 0;
    // sys.executable 设成启动器自己：Android 版 Python 在 sys.executable 为空（嵌在 App 里）时把 stdout / stderr 转去 logcat，
    // 这里是子进程，输出要原样给 captae-core 读（_android_support.init_streams）
    snprintf(path, sizeof(path), "%s/libytdlp.so", libdir);
    check(PyConfig_SetBytesString(&config, &config.executable, path));

    // sys.argv = [yt-dlp 的 zipapp, 参数...]；启动脚本按 sys.argv[0] 跑它
    wchar_t **wargv = PyMem_RawCalloc((size_t)argc + 1, sizeof(wchar_t *));
    wargv[0] = wide(ytdlp);
    for (int i = 1; i < argc; i++) wargv[i] = wide(argv[i]);
    check(PyConfig_SetArgv(&config, argc, wargv));
    check(PyConfig_SetString(&config, &config.run_command, L"import captae_boot; captae_boot.main()\n"));

    check(Py_InitializeFromConfig(&config));
    PyConfig_Clear(&config);
    return Py_RunMain();
}
