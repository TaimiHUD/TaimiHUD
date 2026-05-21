-- stub for testing; core modules are built-in to scripting engine
local util = require"@taimi/util"

local MarkerAttributes = {
	i = {}
}
MarkerAttributes.mt = { __index = MarkerAttributes.i }
function MarkerAttributes:New(i)
	i = util.setmetatable(i or {}, self.mt)
	if i.attrs == nil then
		i.attrs = {}
	end
	return i
end
function MarkerAttributes.new(...)
	return MarkerAttributes:New(...)
end
-- TODO: function MarkerAttributes.copy_from_table(attrs, out)

--[[function MarkerAttributes.i:Append(extra)
	util.table_copy_shallow(extra, self.attrs)
end]]
function MarkerAttributes.i:UnsetAttrByKey(key)
	self.attrs[key] = nil
end
function MarkerAttributes.i:SetAttrByKey(key, value)
	self.attrs[key] = value
end
function MarkerAttributes.i:GetAttrByKey(key)
	return this[key]
end

return {
	MarkerAttributes = MarkerAttributes,
}
