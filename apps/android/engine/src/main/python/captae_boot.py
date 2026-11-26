"""Android 完整版里 yt-dlp 的启动脚本（MIT，Captae 自己写的）。

libytdlp.so（ytdlp_launcher.c）初始化好 Python 以后运行 ``captae_boot.main()``：
1. 装一个查找器：扩展模块（_ssl、_sqlite3、zlib……）打包在原生库目录里，名字是 ``libpy_<模块名>.so``。
   Android 10 起可写目录里的文件不该当代码加载，所以它们不跟标准库一起解压，留在安装器管的原生库目录里。
2. 把 sys.argv[0] 指的 yt-dlp zipapp 当 ``__main__`` 跑（和 ``python yt-dlp ...`` 一样）。
"""

import os
import sys
from importlib.machinery import ExtensionFileLoader, ModuleSpec, PathFinder


class NativeLibFinder:
    """顶层扩展模块 → <原生库目录>/libpy_<模块名>.so"""

    def __init__(self, libdir):
        self.libdir = libdir

    def find_spec(self, name, path=None, target=None):
        # lib-dynload 里的都是顶层模块；包里的子模块交给别的查找器
        if path is not None or '.' in name:
            return None
        f = os.path.join(self.libdir, f'libpy_{name}.so')
        if not os.path.isfile(f):
            return None
        return ModuleSpec(name, ExtensionFileLoader(name, f), origin=f)

    def invalidate_caches(self):
        pass


def install(libdir=None):
    libdir = libdir or os.environ.get('CAPTAE_ENGINE_LIBDIR')
    if not libdir or any(isinstance(f, NativeLibFinder) for f in sys.meta_path):
        return
    finder = NativeLibFinder(libdir)
    # 放在 PathFinder 前面：标准库目录里没有 lib-dynload，先问它是白找
    idx = next((i for i, f in enumerate(sys.meta_path) if f is PathFinder), len(sys.meta_path))
    sys.meta_path.insert(idx, finder)


def main():
    install()
    import runpy
    runpy.run_path(sys.argv[0], run_name='__main__')
