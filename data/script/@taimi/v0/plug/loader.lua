local util = require"@taimi/util"

local export = {}

local Loader = {
	i = {},
	mt = {},
}
function Loader.mt:__index(k)
	return Loader.i[k]
end
function Loader.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Loader.mt)
end
function Loader.i:ResourcePath(subpath)
	error("stub: ResourcePath")
end
function Loader.i:RequireSrc(name)
	error("stub: RequireSrc")
end

export.Loader = {
	for_plug = Loader.for_plug,
}

return export
