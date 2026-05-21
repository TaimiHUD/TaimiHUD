local util = require"@taimi/util"

local export = {}

local DataLink = {
	i = {},
	mt = {},
}
function DataLink.mt:__index(k)
	return DataLink.i[k]
end
function DataLink.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, DataLink.mt)
end

export.DataLink = {
	for_plug = DataLink.for_plug,
}

return export
