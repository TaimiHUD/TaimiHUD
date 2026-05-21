local SEP = "."
local SEP_PAT = "%."
ids = {
	SEP = SEP,
	SEP_PAT = SEP_PAT
}

function ids.safe_name(name)
	return string.gsub(tostring(name), SEP_PAT, "_")
end
function ids.join(parent, name)
	if name == nil then
		error("ID name is nil", 2)
	elseif parent == nil then
		return tostring(name)
	end
	-- return string.format("%s.%s", parent, name)
	return tostring(parent) .. SEP .. tostring(name)
end

function ids.name_of(id)
	id = tostring(id)
	local final_sep = string.find(id, SEP_PAT)
	if final_sep ~= nil then
		id = string.sub(id, -final_sep + 1)
	end
	return id
end
function ids.parent_of(id)
	id = tostring(id)
	local final_sep = string.find(id, SEP_PAT)
	if final_sep == nil then
		return nil
	else
		return string.sub(id, 1, -final_sep - 1)
	end
end
function ids.name_of_split(id)
	id = tostring(id)
	local final_sep = string.find(id, SEP_PAT)
	if final_sep == nil then
		return id, nil
	end
	final_sep = -final_sep
	local name_id = string.sub(id, final_sep + 1)
	local parent_id = string.sub(id, 1, final_sep - 1)
	return name_id, parent_id
end

return ids
