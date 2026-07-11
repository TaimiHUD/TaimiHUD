local export = {
	UrlScheme = "gw2files",
	UrlRootV1 = "v1",
	UrlRootV2 = "v2",
}

-- numeric id
function export.UrlV1(file_id)
	local Url = require"@taimi/url".Url
	return Url.new(("%s://%s/%d"):format(export.UrlScheme, export.UrlRootV1, file_id))
end

-- string id
function export.UrlV2(id)
	local Url = require"@taimi/url".Url
	return Url.new(("%s://%s/%s"):format(export.UrlScheme, export.UrlRootV2, id))
end

-- auto
function export.UrlFromId(id)
	if type(id) == "number" then
		return export.UrlV1(id)
	else
		return export.UrlV2(id)
	end
end

return export
