# Configuration file for the Sphinx documentation builder.
#
# For the full list of built-in configuration values, see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html
from pathlib import Path
import subprocess
from sphinx.application import Sphinx

# -- Project information -----------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#project-information

project = "cpp-linter"
copyright = "2023, Brendan Doherty"
author = "Brendan Doherty"

# -- General configuration ---------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#general-configuration

extensions = [
    "sphinx_immaterial",
]

templates_path = ["_templates"]
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]


# -- Options for HTML output -------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#options-for-html-output

html_theme = "sphinx_immaterial"
html_static_path = ["_static"]
html_logo = "_static/logo.png"
html_favicon = "_static/favicon.ico"
html_css_files = ["extra_css.css"]
html_title = "cpp-linter"

html_theme_options = {
    "repo_url": "https://github.com/2bndy5/cpp_linter_rs",
    "repo_name": "cpp_linter_rs",
    "palette": [
        {
            "media": "(prefers-color-scheme: light)",
            "scheme": "default",
            "primary": "light-blue",
            "accent": "deep-purple",
            "toggle": {
                "icon": "material/lightbulb-outline",
                "name": "Switch to dark mode",
            },
        },
        {
            "media": "(prefers-color-scheme: dark)",
            "scheme": "slate",
            "primary": "light-blue",
            "accent": "deep-purple",
            "toggle": {
                "icon": "material/lightbulb",
                "name": "Switch to light mode",
            },
        },
    ],
    "features": [
        "navigation.top",
        "navigation.tabs",
        "navigation.tabs.sticky",
        "toc.sticky",
        "toc.follow",
        "search.share",
    ],
}
object_description_options = [
    ("std:option", dict(include_fields_in_toc=False)),
]

# -- Parse CLI args from `-h` output -------------------------------------


def setup(app: Sphinx):
    """Generate a doc from the executable script's ``--help`` output."""

    subprocess.run(
        ["cargo", "run", "--example", "cli_doc"],
        check=True,
        cwd=str(Path(__file__).parent.parent.parent / "cpp-linter-lib"),
    )
