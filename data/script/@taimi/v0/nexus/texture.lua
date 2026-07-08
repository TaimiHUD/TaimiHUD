local util = require"@taimi/util"

local export = {}

local Texture = {
	i = {},
	mt = {},
}
function Texture.mt:__index(k)
	return Texture.i[k]
end
function Texture.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Texture.mt)
end
function Texture.i:LoadPath(id, path)
	error("stub: Texture:LoadPath")
end

export.Texture = {
	for_plug = Texture.for_plug,
}

return export
