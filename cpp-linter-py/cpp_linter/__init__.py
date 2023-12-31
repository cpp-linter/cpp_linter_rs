# type: ignore
# ruff: noqa: F405 F403
from .cpp_linter import *


__doc__ = cpp_linter.__doc__
if hasattr(cpp_linter, "__all__"):
    __all__ = cpp_linter.__all__
