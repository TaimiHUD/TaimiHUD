local menu = require"@taimi/core/ui/menu"
local util = require"@taimi/util"
local ud = require"@taimi/util/ud"

local MenuHandle = ud.make_ud_wrapper(menu.MenuHandle, "MenuHandle")
function MenuHandle.s.wrap(m)
	return ud.wrap_instance(MenuHandle, m)
end
ud.wrap_method_into(MenuHandle, "SetAttrByKey")
ud.wrap_method_into(MenuHandle, "GetAttrByKey")
ud.wrap_method_into(MenuHandle, "UnsetAttrByKey")
ud.wrap_method_into(MenuHandle, "GetId")
ud.wrap_method_into(MenuHandle, "GetState")
ud.wrap_method_into(MenuHandle, "SetState")
--[[ud.wrap_method_into(MenuHandle, "Unmask")
ud.wrap_method_into(MenuHandle, "Mask")]]
function MenuHandle.i:Mask()
	-- TODO
end
function MenuHandle.i:Unmask()
	-- TODO
end

local Menu = ud.make_ud_wrapper(menu.Menu, "Menu")
function Menu.s.wrap(menu)
	return ud.wrap_instance(Menu, menu)
end
ud.wrap_method_into(MenuHandle, "GetAttrByKey")
ud.wrap_method_into(Menu, "GenId")
ud.wrap_method_into(Menu, "RemoveId")
ud.wrap_method_into(Menu, "LookupId")
function Menu.i:Register(...)
	local menu = ud.unwrap(self):Register(...)
	return MenuHandle.wrap(menu)
end

-- MenuHandle builder
local MenuItem = {
	i = {},
	mt = {},
	attrkeys = {
		NameId = "name",
		ParentId = "type",
		Name = "displayname",
		Tooltip = "tip-description",
		Checked = "defaulttoggle",
		Checkable = "isseparator",
		Icon = "iconfile",
	},
}
function MenuItem.mt:__index(k)
	local attrname = MenuItem.attrkeys[k]
	if k == "Checkable" then
		return not rawget(self, "attrs"):GetAttrByKey(attrname)
	elseif attrname ~= nil then
		return rawget(self, "attrs"):GetAttrByKey(attrname)
	elseif k == "Id" then
		return self:GetId()
	else
		return MenuItem.i[k]
	end
end
function MenuItem.mt:__newindex(k, v)
	local attrname = MenuItem.attrkeys[k]
	if k == "Checkable" then
		rawget(self, "attrs"):SetAttrByKey(attrname, not v)
	elseif attrname ~= nil then
		rawget(self, "attrs"):SetAttrByKey(attrname, v)
		if k == "NameId" or k == "ParentId" then
			rawset(self, "id", nil)
		end
	elseif k == "Id" then
		self:SetId(v)
	else
		rawset(self, k, v)
	end
end
function MenuItem:New(name_id, i)
	i = i or {}
	if i.attrs == nil then
		local MarkerAttributes = require("@taimi/core/attrs").MarkerAttributes
		i.attrs = MarkerAttributes.new_category()
		i.attrs:SetAttrByKey("name", name_id)
	end

	return util.setmetatable(i, self.mt)
end
function MenuItem:WithId(id, i)
	local ids = require"@taimi/id"
	local name_id, parent_id = ids.name_of_split(id)
	i = self:New(ids.name_of(id), i)
	i.ParentId = parent_id
	return i
end
function MenuItem.new(...)
	return MenuItem:New(...)
end
function MenuItem.with_id(...)
	return MenuItem:WithId(...)
end
function MenuItem.i:GetId(id)
	local id = rawget(self, "id")
	if id == nil then
		local ids = require"@taimi/id"
		id = ids.join(self.ParentId, self.NameId)
		rawset(self, "id", id)
	end
	return id
end
function MenuItem.i:SetId(id)
	local ids = require"@taimi/id"
	local name_id, parent_id = ids.name_of_split(id)
	self.NameId = name_id
	self.ParentId = parent_id
	rawset(self, "id", id)
end
function MenuItem.i:RegisterWith(menu)
	local handle = menu:Register(self:GetId())
	if handle == nil then
		return nil
	end
	for k,attrname in pairs(MenuItem.attrkeys) do
		attrname = self[k]
		if attrname ~= nil and k ~= "NameId" and k ~= "ParentId" then
			handle[k] = attrname
		end
	end
	return handle
end
function MenuItem.i:RemoveFrom(menu)
	menu:Remove(self)
end

function Menu.i:LookupId(...)
	return MenuHandle.wrap(ud.unwrap(self):LookupId(...))
end
function Menu.i:Remove(handle_or_id, ...)
	if type(handle_or_id) ~= "string" then
		handle_or_id = handle_or_id.Id
	end
	self:RemoveId(handle_or_id, ...)
end

MenuHandle.s.attrkeys = MenuItem.attrkeys
function MenuHandle.mt:__index(k)
	local attrname = ud.static_of(self).attrkeys[k]
	if attrname ~= nil then
		return ud.unwrap(self):GetAttrByKey(attrname)
	elseif k == "Checkable" then
		return not ud.unwrap(self):GetAttrByKey("isseparator")
	elseif k == "Checked" then
		return self:GetState()
	elseif k == "Id" then
		return self:GetId()
	else
		return ud.instance_mt.__index(self, k)
	end
end
function MenuHandle.mt:__newindex(k, v)
	local attrname = ud.static_of(self).attrkeys[k]
	if attrname ~= nil then
		ud.unwrap(self):SetAttrByKey(attrname, v)
	elseif k == "Checkable" then
		ud.unwrap(self):SetAttrByKey("isseparator", not v)
	elseif k == "Checked" then
		return self:SetState(v)
	else
		return ud.instance_mt.__newindex(self, k, v)
	end
end

Menu.s.attrkeys = MenuItem.attrkeys
function Menu.mt:__index(k)
	local attrname = ud.static_of(self).attrkeys[k]
	if attrname ~= nil then
		return rawget(self, ud.key_instance):GetAttrByKey(attrname)
	else
		return ud.instance_mt.__index(self, k)
	end
end

return {
	Menu = Menu,
	MenuHandle = MenuHandle,
	MenuItem = MenuItem,
}
