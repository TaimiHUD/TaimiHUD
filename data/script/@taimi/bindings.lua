local bindings = require"@taimi/core/bindings"
local ud = require"@taimi/util/ud"

local Control = bindings.Control
--[[local Control = ud.make_ud_wrapper(bindings.Control, "Control")
ud.wrap_constructor_into(Control, "from_index")]]

local GameControls = ud.make_ud_wrapper(bindings.GameControls, "GameControls")
ud.wrap_constructor_into(GameControls, "new_empty")
ud.wrap_method_into(GameControls, "Bits32At0")
ud.wrap_method_into(GameControls, "IsEmpty")
ud.wrap_method_into(GameControls, "GetAt0")
ud.wrap_method_into(GameControls, "SetAt0")
ud.wrap_method_into(GameControls, "NextIndexFrom0")
function GameControls.i:iter()
	return GameControls.i.NextIndexFrom0, self, nil
end

-- local ControlNames = util.table_map_value(bindings.ControlNames, Control.wrap)
local ControlNames = bindings.ControlNames

return {
	Control = Control,
	GameControls = GameControls,
	ControlNames = ControlNames,
}
