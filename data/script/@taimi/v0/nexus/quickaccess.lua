local util = require"@taimi/util"

local export = {}

local QuickAccess = {
	i = {},
	mt = {},
}
function QuickAccess.mt:__index(k)
	return QuickAccess.i[k]
end
function QuickAccess.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, QuickAccess.mt)
end

export.QuickAccess = {
	for_plug = QuickAccess.for_plug,
}

return export
