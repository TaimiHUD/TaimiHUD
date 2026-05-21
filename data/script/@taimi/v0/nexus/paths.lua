local util = require"@taimi/util"

local export = {}

local Paths = {
	i = {},
	mt = {},
}
function Paths.mt:__index(k)
	return Paths.i[k]
end
function Paths.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Paths.mt)
end

export.Paths = {
	for_plug = Paths.for_plug,
}

return export
