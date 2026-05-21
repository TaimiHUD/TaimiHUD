local util = require"@taimi/util"

local export = {}

local Bind = {
	i = {},
	mt = {},
}
function Bind.mt:__index(k)
	return Bind.i[k]
end
function Bind.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Bind.mt)
end

export.Bind = {
	for_plug = Bind.for_plug,
}

return export
