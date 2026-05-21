local util = require"@taimi/util"

local Menu = {
	i = {},
	mt = {},
}
function Menu:NewEmpty(pack_info, id, i)
	local menus = require"@taimi/ui/menu"
	i = i or {}
	rawset(i, "pack_info", pack_info)
	if i.handle == nil then
		if id == nil then
			id = pack_info:GetRootMenu():GenId()
		end
		rawset(i, "item", menus.MenuItem.new(id))
	end

	return util.setmetatable(i, self.mt)
end
function Menu:New(pack_info, id, name, onclick, checkable, state, tooltip)
	local menus = require"@taimi/ui/menu"
	local menu = self:NewEmpty(pack_info, id)
	menu.Name = name
	if onclick ~= nil then
		menu.OnClick = onclick
	end
	menu.CanCheck = checkable and true
	if checkable or checked then
		menu.Checked = checked and true
	end
	if tooltip ~= nil then
		menu.Tooltip = tooltip
	end
	return menu
end
function Menu.new(...)
	return Menu:New(...)
end
function Menu.new_empty(...)
	return Menu:NewEmpty(...)
end
function Menu.mt:__index(k)
	local v = Menu.i[k]
	if v ~= nil then
		return v
	elseif k == "OnClick" then
		return self:GetOnClick()
	else
		return Menu.AsAttrs(self)[k]
	end
end
function Menu.mt:__newindex(k, v)
	if k == "OnClick" then
		self:SetOnClick(v)
	else
		Menu.AsAttrs(self)[k] = v
	end
end
function Menu.mt:__tostring()
	return self.Id
end
--[[function Menu.i:GetChecked()
	if self.cat ~= nil then
		return self.cat:IsVisible()
	else
		return self.attrs:GetAttrByKey("defaulttoggle")
	end
end]]
--[[function Menu.i:SetChecked(state)
	if self.cat == nil then
		self.attrs:SetAttrByKey("defaulttoggle", state)
	elseif state then
		self.cat:Show()
	else
		self.cat:Hide()
	end
end]]
function Menu.i:SetOnClick(cb)
	if self.handle == nil then
		rawset(self, "onclick", cb)
	else
		local prev = rawget(self, "onclick")
		rawset(self, "onclick", cb)
		if cb == prev then
			return
		end
		if cb ~= nil then
			self:UnmaskOnClick()
		else
			self:MaskOnClick()
		end
	end
end
function Menu.i:UnmaskOnClick()
	local event = require"@taimi/core/event"
	local f = util.fun_bind_method0("HandleClick", self)
	self.pack_events:RegisterMarkerFunc(event.HostSignal.MenuClick, self.Id, f)
	if self.handle ~= nil then
		self.handle:Unmask()
	end
end
function Menu.i:MaskOnClick()
	local event = require"@taimi/core/event"
	if self.handle ~= nil then
		self.handle:Mask()
	end
	self.pack_events:RegisterMarkerFunc(event.HostSignal.MenuClick, self.Id, nil)
end
function Menu.i:HandleClick(_e, _ev, ...)
	local onclick = self:GetOnClick()
	if onclick ~= nil then
		return onclick(self, ...)
	end
end
function Menu.i:GetOnClick()
	return rawget(self, "onclick")
end
function Menu.i:Add(...)
	local root = self:GetRoot()
	local name = ...
	local id = root:GenId(self.Id, name)
	local menu = Menu.new(self.pack_info, id, ...)
	rawset(menu, "pack_events", self.pack_events)
	return Menu.AddToRoot(menu, root)
end
function Menu.i:Remove(child)
	local root = self:GetRoot()
	root:Remove(child, true)
end
function Menu.i:GetRoot()
	local menus = require"@taimi/ui/menu"
	return menus.Menu.wrap(self.pack_info:GetRootMenu())
end
function Menu.AddToRoot(self, menu)
	if self.handle ~= nil then
		-- TODO: reuse item and return a copy?
		return self
	end
	local handle = self.item:RegisterWith(menu)
	if handle == nil then
		return nil
	end
	rawset(self, "handle", handle)
	rawset(self, "item", nil)
	if rawget(self, "onclick") ~= nil then
		self:UnmaskOnClick()
	end
	return self
end
function Menu.AsAttrs(self)
	return rawget(self, "handle") or rawget(self, "item")
end
-- function Menu.i:Focus() return self.cat:Focus() end
-- function Menu.i:Unfocus() return self.cat:Unfocus() end
-- function Menu.i:Interact() return self.cat:Interact() end
--[[function Menu.i:Populate()
	local ud = require"@taimi/util/ud"
	self.cat = self.pack_info:GetPackHandle():CreateCategory(self.id, ud.unwrap(self.attrs))
end
function Menu.i:AppendTo(parent)
	parent:AppendChild(self)
end
function Menu.i:AppendChild(submenu)
	if submenu.cat ~= nil then
		error("menu duplicated")
	end
	table.insert(self.children, submenu)
	submenu.id = (self.id or self.Name) .. "." .. submenu.Name
	submenu:Populate()
end
function Menu.i:RemoveFrom(parent)
	parent:Remove(self)
end
function Menu.i:Remove(submenu)
	local removed
	for i, m in ipairs(self.children) do
		if m == submenu then
			removed = util.array_remove_at(self.children, i)
			break
		end
	end
	if not removed then
		require("@taimi/core/log").Info(string.format("menu %s was not removed from %s", submenu, self))
	end
	if submenu.cat ~= nil then
		submenu.cat:Remove()
	end
	submenu.cat = nil
end]]

local RootMenu = {
	i = {},
	mt = {},
	attrs = {
		NameId = "name",
		Id = "name",
		Name = "displayname",
		Tooltip = "tip-description",
	},
}
function RootMenu:New(pack_info, pack_events, i)
	i = i or {}
	rawset(i, "pack_info", pack_info)
	rawset(i, "pack_events", pack_events)
	--[[if i.children == nil then
		rawset(i, "children", {})
	end]]

	return util.setmetatable(i, self.mt)
end
function RootMenu.new(...)
	return RootMenu:New(...)
end
function RootMenu.mt.__index(t, k)
	local attr = RootMenu.attrs[k]
	if attr ~= nil then
		local root = t.pack_info:GetRootMenu()
		if root == nil then
			return nil
		elseif k == "Tooltip" then
			return root:GetAttrByKey("tip-name") or root:GetAttrByKey(attr)
		-- elseif root.GetAttrByKey == nil then return root[k]
		-- elseif root.GetAttrByKey == nil then return root[k]
		else
			return root:GetAttrByKey(attr)
		end
	elseif k == "CanCheck" then
		local root = t.pack_info:GetRootMenu()
		if root == nil then
			return nil
		elseif root.GetAttrByKey ~= nil then
			return not root:GetAttrByKey("isseparator")
		else
			return root[k]
		end
	elseif k == "Checked" then
		return t:GetChecked()
	elseif k == "OnClick" then
		return t:GetOnClick()
	end
	return RootMenu.i[k]
end
function RootMenu.mt:__tostring()
	return self.Id
end
--[[function RootMenu.i:GetChecked()
	return self.pack_info:GetRootMenu().Checked
end
function RootMenu.i:GetOnClick()
	return rawget(self, "onclick")
end
function RootMenu.i:Add(...)
	require("@taimi/core/log").Info("TODO: RootMenu:Add")
	local menu = Menu.new(self.pack_info, ...)
	menu:AppendTo(self)
	return menu
end
function RootMenu.i:AppendChild(...)
	Menu.i.AppendChild(self, ...)
end
function RootMenu.i:Remove(...)
	require("@taimi/core/log").Info("TODO: RootMenu:Remove")
	Menu.i.Remove(self, ...)
end]]
function RootMenu.i:Add(...)
	local menus = require"@taimi/ui/menu"
	local root = menus.Menu.wrap(self.pack_info:GetRootMenu())
	local name = ...
	local id = root:GenId(self.Id, name)
	local menu = Menu.new(self.pack_info, id, ...)
	rawset(menu, "pack_events", self.pack_events)
	return Menu.AddToRoot(menu, root)
end
function RootMenu.i:Remove(menu)
	local root = menus.Menu.wrap(self.pack_info:GetRootMenu())
	root:Remove(menu, true)
end

return {
	Menu = Menu,
	RootMenu = RootMenu,
}
