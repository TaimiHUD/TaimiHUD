local util = require"@taimi/util"

local Debug = {
	formatters = {
		fallback = util.id,
	},
	i = {},
}
Debug.mt = { __index = Debug.i }
function Debug:New(i)
	i = i or {}
	if i.registry == nil then
		i.registry = {}
	end
	i.formatters = util.alias_index_to(self.formatters, i.formatters)
	return util.setmetatable(i, self.mt)
end
function Debug.new(...)
	return Debug:New(...)
end
function Debug.i:Watch(k, v)
	self.registry[k] = v
end
function Debug.i:ClearWatch(k)
	self.registry[k] = nil
end
function Debug.i:GetWatches()
	return self.registry
end
function Debug.i:ExportWatches(o)
	o = o or {}
	for k,v in pairs(self.registry) do
		self:FormatValueInto(o, k, v)
	end
	return o
end
function Debug.i:FormatValueInto(out, k, v)
	out[k] = self:FormatValue(k, v)
end
function Debug.i:FormatValue(k, v)
	if v == nil then
		v = self.registry[k]
	end
	local formatter = util.is_indexable(v) and v.__todebugwatch or self.formatters[util.typeof(v)];
	return (formatter or self.formatters.fallback)(v)
end

return {
	Debug = Debug,
}
