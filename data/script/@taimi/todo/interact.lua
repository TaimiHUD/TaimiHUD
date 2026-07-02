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

export.FilterKind = {
	Behaviour = 1,
	Achievement = 2,
	Festival = 3,
	MapType = 4,
	Mount = 5,
	Race = 6,
	Schedule = 7,
	Profession = 8,
	Specialization = 9,
	Raid = 10,
	Script = 11,
}
export.FilterMask = {}
for k,v in pairs(export.FilterKind) do
	export.FilterMask[k] = bitop.lshift(1, v)
end

return export
