//! Module `lua-utf8` attendu par PoB (luautf8 en C dans PoB pour Windows).
//!
//! PoB ne s'en sert que pour les séparateurs de milliers et les champs de saisie :
//! les fonctions de `string` suffisent, les textes manipulés sans fenêtre sont en ASCII.

use mlua::{Lua, Table};

const MODULE: &str = r#"
local u = {}
u.find = string.find
u.gsub = string.gsub
u.match = string.match
u.gmatch = string.gmatch
u.sub = string.sub
u.len = string.len
u.lower = string.lower
u.upper = string.upper
u.reverse = string.reverse
u.byte = string.byte
u.char = string.char
u.next = function(s, i)
	i = (i or 0) + 1
	if i > #s then return nil end
	return i
end
return u
"#;

pub fn installer(lua: &Lua) -> mlua::Result<()> {
    let module: Table = lua.load(MODULE).set_name("lua-utf8").eval()?;
    let preload: Table = lua.globals().get::<Table>("package")?.get("preload")?;
    preload.set(
        "lua-utf8",
        lua.create_function(move |_, ()| Ok(module.clone()))?,
    )?;
    Ok(())
}
