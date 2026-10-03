-- Stand-in for the `R8bGf2Core.FunsExternal` module that
-- `dev/active/34d85cb9/extraction/A8b_lean/Funs.lean:5` imports.
--
-- Aeneas emits that import unconditionally under `-split-files`, but the A8b
-- run generated no `FunsExternal_Template.lean` beside it, so no generated
-- file supplies the module. The A8b extraction declares no external function,
-- so the module is empty apart from the two imports the generated files need.
--
-- This file is an elaboration-harness input, not proof code, and it is not
-- part of any lake target.
import Aeneas
import R8bGf2Core.Types
