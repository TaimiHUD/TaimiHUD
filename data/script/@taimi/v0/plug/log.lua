local util = require"@taimi/util"

local export = {}

local Log = {
	i = {
		logger = require"@taimi/core/log",
	},
	mt = {},
}
function Log.mt:__index(k)
	return Log.i[k]
end
function Log.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Log.mt)
end
function Log.i:Prefix()
	return ("%s; "):format(self.plug)
end
function Log.i:Error(...)
	self.logger.Error(self:Prefix(), ...)
end
function Log.i:Warn(...)
	self.logger.Warn(self:Prefix(), ...)
end
function Log.i:Info(...)
	self.logger.Info(self:Prefix(), ...)
end
function Log.i:Debug(...)
	self.logger.Debug(self:Prefix(), ...)
end
function Log.i:Trace(...)
	self.logger.Trace(self:Prefix(), ...)
end

export.Log = {
	for_plug = Log.for_plug,
}

return export
