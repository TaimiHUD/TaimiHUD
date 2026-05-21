local util = require"@taimi/util"

local export = {}

local RtApi = {
	i = {},
	mt = {},
	Signature = 0x2501a02c,
	-- DL_RTAPI
	ResourceId = "RTAPI",
}
function RtApi.mt:__index(k)
	return RtApi.i[k]
end
function RtApi.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, RtApi.mt)
end

export.RtApi = {
	for_plug = RtApi.for_plug,
	Signature = RtApi.Signature,
	ResourceId = RtApi.ResourceId,
}

return export
