local util = require"@taimi/util"

local export = {}

local Mumble = {
	i = {},
	mt = {},
}
function Mumble.mt:__index(k)
	local v = Mumble.i[k]
	if v == nil then
		v = rawget(self, "mumb")
		if v == nil then
			v = require"@taimi/core/mumblelink"
			rawset(self, "mumb", v)
		end
		v = v[k]
	end
	return v
end
function Mumble.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Mumble.mt)
end

export.Mumble = {
	for_plug = Mumble.for_plug,
}

return export
