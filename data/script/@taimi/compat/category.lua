local util = require"@taimi/util"
local ud = require"@taimi/util/ud"

local Category = {
	i = {},
	mt = {},
	attrs = {
		IsHidden = "ishidden",
		IsSeparator = "isseparator",
		DisplayName = "displayname",
		DefaultToggle = "defaulttoggle",
		Name = "name",
		-- Namespace = "type",
	},
	-- TODO: source from core/builtin
	tag_type = 3,
}
function Category.mt.__index(t, k)
	local attr = Category.attrs[k]
	if attr ~= nil then
		return rawget(t, ud.key_instance):GetAttrByKey(attr)
	-- TODO: elseif k == "Root" then return rawget(t, ud.key_instance):GetAttrByKey("type") == nil
	elseif k == "Parent" then
		return t:GetParent()
	end
	local v = Category.i[k]
	if v ~= nil then
		return v
	end
	return rawget(t, ud.key_instance)[k]
end
function Category.mt.__newindex(t, k, v)
	local c_i = ud.unwrap(t)
	local attr = Category.attrs[k]
	if attr ~= nil then
		return c_i:SetAttrByKey(attr, v)
	else
		c_i[k] = v
	end
end
function Category.mt.__tostring(t)
	return tostring(ud.unwrap(t))
end
function Category:Wrap(cat, pack_info, i)
	i = i or {}
	i.pack_info = pack_info
	i[ud.key_instance] = cat
	return util.setmetatable(i, self.mt)
end
function Category.wrap(...)
	return Category:Wrap(...)
end
function Category.i:Remove()
	self.pack_info:GetPackHandle():RemoveCategory(ud.unwrap(self))
end
function Category.i:GetMarkers(recursive)
	local out
	if recursive then
		out = self.pack_info:GetWorldHandle():MarkersUnderCategory(self.Namespace)
	else
		out = self.pack_info:GetWorldHandle():MarkersInCategory(self.Namespace)
	end
	local Poi = require("@taimi/compat/poi").Poi
	return util.table_map_collect(
		function(m) return Poi.wrap(m, self.pack_info) end,
		out)
end
function Category.i:GetTrails(recursive)
	local out
	if recursive then
		out = self.pack_info:GetWorldHandle():TrailsUnderCategory(self.Namespace)
	else
		out = self.pack_info:GetWorldHandle():TrailsInCategory(self.Namespace)
	end
	local Trail = require("@taimi/compat/trail").Trail
	return util.table_map_collect(
		function(m) return Trail.wrap(m, self.pack_info) end,
		out)
end
function Category.i:GetChildren(recursive)
	local out
	if recursive then
		out = self.pack_info:GetPackHandle():CategoryDescendents(ud.unwrap(self))
	else
		out = self.pack_info:GetPackHandle():CategoryChildren(ud.unwrap(self))
	end
	return util.table_map_collect(
		function(c) return Category.wrap(c, self.pack_info) end,
		out)
end
function Category.i:GetParent()
	-- TODO? local parent = self[ud.key_instance].Parent
	local parent = self:GetAttrByKey("type")
	if type(parent) == "string" then
		parent = self.pack_info:CategoryByType(parent)
	end
	if parent ~= nil then
		parent = Category.wrap(parent, self.pack_info)
	end
	return parent
end
function Category.i:IsVisible(...)
	return ud.unwrap(self):IsVisible(...)
end
function Category.i:Show(...)
	return ud.unwrap(self):Show(...)
end
function Category.i:Hide(...)
	return ud.unwrap(self):Hide(...)
end

local RootCategory = {
	i = {},
	mt = {},
}
function RootCategory.mt.__index(t, k)
	if k == "Parent" then
		return nil
	end
	local v = RootCategory.i[k]
	if v ~= nil then
		return v
	else
		return Category.mt.__index(t, k)
	end
end
function RootCategory.mt.__newindex(t, k, v)
	return Category.mt.__newindex(t, k, v)
end
function RootCategory.mt.__tostring(t)
	return Category.mt.__tostring(t)
end
function RootCategory:New(pack_info, i)
	i = Category.wrap(pack_info:GetRootCategory(), pack_info, i)
	return util.setmetatable(i, self.mt)
end
function RootCategory.new(...)
	return RootCategory:New(...)
end
function RootCategory.i:GetOrAddCategoryFromNamespace(id)
	local cat = self.pack_info:CategoryByType(id)
	if cat == nil then
		local MarkerAttributes = require("@taimi/pack/attrs").MarkerAttributes;
		local a = MarkerAttributes.new_category()
		cat = self.pack_info:GetPackHandle():CreateCategory(id, ud.unwrap(a))
	end
	if cat ~= nil then
		cat = Category.wrap(cat, self.pack_info)
	end
	return cat
end
function RootCategory.i:GetRoots()
	local out = self.pack_info:GetPackHandle():CategoryRoots()
	return util.table_map_collect(
		function(c) return Category.wrap(c, self.pack_info) end,
		out)
end

return {
	Category = Category,
	RootCategory = RootCategory,
}
