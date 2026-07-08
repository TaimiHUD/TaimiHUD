-- TODO: implement as builtin x.x
-- stub for testing; core modules are built-in to scripting engine
local bitop = require("@taimi/bitop")

local export = {}

export.TriggerKind = {
	Behaviour = 1,
	Copy = 2,
	Reset = 3,
	Toggle = 4,
	Show = 5,
	Hide = 6,
	Script = 7,
	Bounce = 8,
}

export.TriggerMask = {}
for k,v in pairs(export.TriggerKind) do
	export.TriggerMask[k] = bitop.lshift(1, v)
end

return export
