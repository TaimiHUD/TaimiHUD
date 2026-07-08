local attrs = require"@taimi/core/attrs"
local ud = require"@taimi/util/ud"

-- TODO: wrap MarkerAttributes and implement some compat stuff here
-- (plus case-sensitivity fixes?)
-- local MarkerAttributes = attrs.MarkerAttributes
local MarkerAttributes = ud.make_ud_wrapper(attrs.MarkerAttributes, "MarkerAttributes")
ud.wrap_constructor_into(MarkerAttributes, "new")
if MarkerAttributes.new_poi ~= nil then
	ud.wrap_constructor_into(MarkerAttributes, "new_poi")
else
	MarkerAttributes.s.new_poi = MarkerAttributes.new
end
if MarkerAttributes.new_trail ~= nil then
	ud.wrap_constructor_into(MarkerAttributes, "new_trail")
else
	MarkerAttributes.s.new_trail = MarkerAttributes.new
end
if MarkerAttributes.new_category ~= nil then
	ud.wrap_constructor_into(MarkerAttributes, "new_category")
else
	MarkerAttributes.s.new_category = MarkerAttributes.new
end
MarkerAttributes.s.attr_key_map = {
	AutoTrigger = "autoTrigger",
	TriggerRange = "triggerRange",
	Category = "type",
}
ud.wrap_method_into(MarkerAttributes, "SetAttrByKey")
ud.wrap_method_into(MarkerAttributes, "GetAttrByKey")
ud.wrap_method_into(MarkerAttributes, "UnsetAttrByKey")
function MarkerAttributes.s:CanonKey(k)
	return self.attr_key_map[k] or string.lower(k)
end
function MarkerAttributes.i:SetAttr(k, v)
	k = ud.static_of(self):CanonKey(k)
	if k == "type" or k == "name" or k == "iconfile" or k == "texture" or k == "traildata" then
		v = tostring(v)
	elseif type(v) == "table" then
		v = ud.unwrap(v)
	end
	self:SetAttrByKey(k, v)
end
function MarkerAttributes.i:GetAttr(k)
	k = ud.static_of(self):CanonKey(k)
	self:GetAttrByKey(canon_k)
end
function MarkerAttributes.i:UnsetAttr(k)
	k = ud.static_of(self):CanonKey(k)
	self:UnsetAttrByKey(canon_k)
end

return {
	MarkerAttributes = MarkerAttributes,
}
