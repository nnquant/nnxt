"""Generate API documentation pages for mkdocs."""

from pathlib import Path

import mkdocs_gen_files

# Python 包路径
PACKAGE_PATH = Path("py-nnxt/python/nnxt")

# 输出目录
OUTPUT_DIR = "api/python"


def main():
    """生成 API 文档页面。"""
    nav = mkdocs_gen_files.Nav()

    # 扫描 Python 模块
    for path in sorted(PACKAGE_PATH.rglob("*.py")):
        # 跳过私有模块
        if path.name.startswith("_") and path.name != "__init__.py":
            continue

        # 计算模块路径
        module_path = path.relative_to(PACKAGE_PATH.parent)
        doc_path = path.relative_to(PACKAGE_PATH.parent).with_suffix(".md")
        full_doc_path = Path(OUTPUT_DIR) / doc_path

        # 构建模块名
        parts = list(module_path.with_suffix("").parts)
        if parts[-1] == "__init__":
            parts = parts[:-1]
            doc_path = doc_path.with_name("index.md")
            full_doc_path = full_doc_path.with_name("index.md")

        if not parts:
            continue

        module_name = ".".join(parts)

        # 添加到导航
        nav[parts] = doc_path.as_posix()

        # 生成文档页面
        with mkdocs_gen_files.open(full_doc_path, "w") as fd:
            fd.write(f"# {module_name}\n\n")
            fd.write(f"::: {module_name}\n")

        # 设置编辑路径
        mkdocs_gen_files.set_edit_path(full_doc_path, path)

    # 生成导航文件
    with mkdocs_gen_files.open(f"{OUTPUT_DIR}/SUMMARY.md", "w") as nav_file:
        nav_file.writelines(nav.build_literate_nav())


if __name__ == "__main__":
    main()
