-- stub for testing; core modules are built-in to scripting engine
local util = require"@taimi/util"

local PackAssets = {
	i = {}
}
PackAssets.mt = { __index = PackAssets.i }
function PackAssets:New(info, i)
	i = util.setmetatable(i or {}, self.mt)
	i.info = info
	return i
end
function PackAssets.new(...)
	return PackAssets:New(...)
end

function PackAssets.i:Require(path)
	error("stub: require", 2)
end
function PackAssets.i:OpenTexture(path)
	error("stub: OpenTexture", 2)
end

local Pack = {
	i = {}
}
Pack.mt = { __index = Pack.i }
function Pack:New(info, i)
	i = util.setmetatable(i or {}, self.mt)
	i.info = info
	return i
end
function Pack.new(...)
	return Pack:New(...)
end

function Pack.i:CreateMarker(attrs)
	error("stub: CreateMarker", 2)
end
function Pack.i:RemoveMarker(poi)
	error("stub: RemoveMarker", 2)
end
function Pack.i:CreateTrail(attrs)
	error("stub: CreateTrail", 2)
end
function Pack.i:RemoveTrail(trail)
	error("stub: RemoveTrail", 2)
end
function Pack.i:CreateCategory(id, attrs)
	error(string.format("stub: CreateCategory(%s)", id), 2)
end
function Pack.i:RemoveCategory(cat)
	error("stub: RemoveCategory", 2)
end

local World = {
	i = {}
}
World.mt = { __index = World.i }
function World:New(info, i)
	i = util.setmetatable(i or {}, self.mt)
	i.info = info
	return i
end
function World.new(...)
	return World:New(...)
end

function World.i:MarkerByGuid(guid)
	error("stub: MarkerByGuid", 2)
end
function World.i:TrailByGuid(guid)
	error("stub: TrailByGuid", 2)
end
function World.i:PathableByGuid(guid)
	error("stub: PathableByGuid", 2)
end
function World.i:MarkersByGuid(guid)
	error("stub: MarkersByGuid", 2)
end
function World.i:TrailsByGuid(guid)
	error("stub: TrailsByGuid", 2)
end
function World.i:PathablesByGuid(guid)
	error("stub: PathablesByGuid", 2)
end

local Space = {
	i = {}
}
Space.mt = { __index = Space.i }
function Space:New(info, i)
	i = util.setmetatable(i or {}, self.mt)
	i.info = info
	return i
end
function Space.new(...)
	return Space:New(...)
end

function Space.i:GetClosestMarker(filtered)
	error("stub: GetClosestMarker", 2)
end
function Space.i:GetClosestMarkers(filtered)
	error("stub: GetClosestMarkers", 2)
end

-- stub for mocking
local PackInfo = {
	i = {}
}
PackInfo.mt = { __index = PackInfo.i }
function PackInfo.i:GetPackAssets()
	return PackAssets.new(self)
end
function PackInfo.i:GetStorage()
	local store = require"@taimi/core/store"
	return store.Storage.new(self)
end
function PackInfo.i:GetPackHandle()
	return Pack.new(self)
end
function PackInfo.i:GetWorldHandle()
	return World.new(self)
end
function PackInfo.i:GetSpaceHandle()
	return Space.new(self)
end
function PackInfo.i:GetRootCategory()
	return Category:NewRoot(self)
end
function PackInfo.i:GetRootMenu()
	local menu = require"@taimi/core/menu"
	menu.Menu.new(self)
end
function PackInfo.i:CategoryByType(id)
	error("stub: CategoryByType", 2)
end
function PackInfo.i:CategoryRoots(cat)
	error("stub: CategoryRoots", 2)
end
function PackInfo.i:CategoryChildren(cat)
	error("stub: CategoryChildren", 2)
end
function PackInfo.i:CategoryDescendents(cat)
	error("stub: CategoryDescendents", 2)
end

return {
	Pack = Pack,
	PackInfo = PackInfo,
}
