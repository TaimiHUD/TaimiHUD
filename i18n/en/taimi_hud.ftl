## Common

join-discord = Join our Discord
discord-link = "https://discord.gg/FycK2nZKQT"
having-issues = If you're having issues with TaimiHUD, feel free to reach out on our Discord or by GitHub issues!
height = Height
font = Font
okay = OK
delete = Delete
copy = Copy
copy-arg = Copy { $arg }
save = Save
quit = Quit
delete-item = { delete } "{ $item }"?
save-item = { save } "{ $item }"?
save-standalone = { save } as a new file
save-append = Append to an existing file
save-edit = { save } edited changes
save-edit-item = { save-edit } to "{ $item }"?
save-mode = Save mode
error = Error
remove = Remove
unknown = Unknown
update = Update
auto-update = Auto-update
always = Always
ask = Ask
never = Never
unset = Unset
add = Add
create-arg = Create new { $arg }
not-create-arg = Use existing { $arg }
description = Description
location = Location: { $path }
data = Data
object = Object
files = Files
clear = Clear
refresh-files = Refresh files
# as in 3D
model = Model
revert = Revert
close = Close
name = Name
icon = Icon
path = Path
title = Title
menu = Menu
controls = Controls
id = ID
category = Category
id-arg = { id }: { $id }
map-id = Map { id }
map-id-arg = { map-id }: { $id }
author = Author
position = position
position_cap = Position
not-applicable = N/A
rt-api-required-base = RTAPI is required for
rt-api-required = { rt-api-required-base } { $reason }.
no-description = No description provided.
no-thing-arg = No { $thing } provided.
expand-all = Expand All
filetype = File Type
filename = File Name
collapse-all = Collapse All
active = Active
inactive = Inactive
enable = Enable
cancel = Cancel
default = Default
disable = Disable
enabled = { enable }d
disabled = { disable }d
author-arg = { author }: { $author }
reset = Reset
timer = Timer
timers = { timer }s
experimental-notice = Hi! This feature is (mostly) experimental. Some things may be confusing and it might require more thought and effort to use than the less experimental features. My apologies for any problems you have; feel free to reach out on Discord. - Kat
name-empty = Name empty.
no-trigger = No trigger position provided.
no-category = No category provided.
map-id-wrong = Map ID incorrect.
no-positions = No marker positions provided.
validation-fail = Validation failed due to:
filename-empty = No filename provided.
count = Count
actions = Actions
module = Module
unspecified = Unspecified

## Addon

addon = Addon
primary-window-toggle = Taimi Window Toggle
context-menu-primary = { menu }
timer-window-toggle = Timer Window Toggle
marker-window-toggle = Marker Window Toggle
pathing-window-toggle = Pathing Window Toggle
pathing-render-toggle = Toggle pathing render
pathing-render-minimap-toggle = Toggle minimap pathing
pathing-render-map-toggle = Toggle map pathing
primary-window-toggle-text = Show/hide taimi primary window
timer-key-trigger = Timer Key Trigger { $id }
timer-key-reset = Reset Timers

## Config

config-tab = Config
stock-imgui-progress-bar = Stock Imgui Progress Bar
shadow = Shadow
centre-text-after-icon = Centre text after icon
imgui-notice = You can control-click on a slider element, or such, to be able to directly input data to it. Remember to press enter after inputting the value.
context-click-notice = Right-click for more options
dpi-scaling = DPI Scaling
dpi-notice = Ensure this matches the {dpi-scaling} setting under the game's Graphics Options in order for map elements to display correctly.
marker-trigger = Marker set position trigger behaviour
marker-condition = Behaviour condition
autoplace-warning = If you do not have RTAPI installed, we will not be able to detect whether you are a lieutenant instead of just a commander.
nexus-quick-access = Quick Access Icons
icon-style = Icon Style
icon-style-plain = Plain
icon-style-scanlines-1 = Scanlines
preferred-loader = Loader Preference
preferred-updater = Update Host Preference
gh-api-token = GitHub API Token
gh-api-token-notice = Rate limit errors when updating datasources may be avoided by configuring a personalized token - only provide if you understand the implications of doing so!
language = Language
addonbinds = Shortcuts
gamebinds = Game Bindings
keybind = Keybind
gamebind-notice = Set these to match your Controls settings in-game. These may be automatically detected when arcdps-unofficial-extras is installed.
precise-markers = Precise Markers
bind = Bind
press-key = press a key

## Windows

primary-window = TaimiHUD
timers-window = Encounter Timers
markers-window = Squad Markers
pathing-window = Pathing Packs
# deprecated(?) aliases
timer-window = { timers-window }
marker-window = { markers-window }

## Modals

addon-uninstall-modal-title = Uninstall { $source }?
addon-uninstall-modal-button = Uninstall
addon-uninstall-modal-description = Please be careful! This will delete the folder and anything it contains.
delete-markerset-warning = Please be careful! This will delete the marker set entry within the file.
overwrite-markerset = Please be careful! This will overwrite the marker set entry within the file.
## Openable

open-button = Open { $kind }
open-error = { error } opening { $kind }: { $path }

## Data sources

intro-to-data-sources = Please make sure you refresh the repository before checking for updates.
data-sources = Data Sources
data-sources-tab = { data-sources }
data-source-repo-update = Refresh sources
data-source-repo-update-tooltip = Fetch the upstream data sources repository to see downloadable items.
checking-for-updates = Checking for updates...
downloading-update = Downloading update...
check-for-updates = Check for updates
check-for-updates-tooltip = Check for updates to any data sources. We don't do this automatically to respect your choice on whether or not to make network requests.
checked-for-updates-last = Last checked for updates at: { $time }
reload-data-sources = Reload data sources
reload-data-sources-tooltip = Reload items from currently installed data sources. Useful if you have changed the files within them!

remote = Remote
update-status = Update Status
version-installed = Installed version: { $version }
version-not-installed = Not installed
update-unknown = Update status unknown; check for updates?
update-not-required = Update not required; up to date!
update-available = New version available: { $version }!
update-error = { error } updating: { $error }!
download = Download
install = Install
attempt-update = Attempt to update anyway?
settings-unloaded = Settings have not yet loaded!
available = Available
up-to-date = Up to date!

## Info tab

info-tab = Info
keybind-triggers = If you need keybind-based timer triggers, please bind the appropriate keys in the loader settings.
active-timer-phases = Active timer phases
phase = Phase
# As in, like, "game engine" or "rendering engine" :o
engine = Engine
ecs-data = ECS { data }
object-data = { object } { data }
object-kind = { object } Kind
model-files = { model } Files
vertices = Vertices
textures = Textures: { $count }
alloc-size = Allocations: { $size }
d3d-textures = D3D Textures: { $count }
size-frag = { $size } { $suffix }
#size-frag-mb = { size-frag(suffix: "MB", size: "$size") }
size-frag-mb = { $size } MB
size-frag-kb = { $size } KB

## Arc

arcdps = ArcDPS
arcdps-tab = { arcdps }
nexus = Nexus

## Markers tab

reload-markers = Reload { markers }
marker-tab = { marker-window }
pathing-tab = { pathing-config }
marker = Marker
markers = { marker }s
markers-place = Place { markers }
marker-set = { marker } Set
marker-set-create = Create { marker-set }
marker-set-edit = Edit { marker-set }
marker-set-delete = Delete { marker-set }
scaling-factor = scaling factor
current-scaling-factor = Current { scaling-factor }: ({ $x }, { $y })
current-scaling-factor-multiple = Current { scaling-factor } as multiple of ft per continent unit: ({ $x }, { $y })
scaling-factor-reset = { reset } detected { scaling-factor }
no-file-associated = Couldn't find associated file
markers-arg = { markers }: { $count }
marker-type = { marker } Type
local-header = Local (XYZ)
map-header = Map (XY)
screen-header = Screen (XY)
marker-not-on-screen = Not on screen
select-a-marker = Please select a marker to configure!
marker-filetype-explanation = There are three kinds of markers file, there is the kind that
  comes with the BlishHUD Commander's Markers module (integrated), there is the kind that they use to ship Community Markers and then there is my own format, which takes the per marker set format and makes it a single file per marker set.
no-markers-for-map = No markers found for current map.
cant-place-markers = Can't place
autoplacement-disable = Disable auto-placement
autoplacement-enable = Enable auto-placement
always-do-action = Always do action
do-action-if-commander = Do action if commander
do-action-if-lieutenant = Do action if lieutenant or commander
never-do-action = Never do action
open-markers-window = Open the markers window
place-markers-automatically = Place markers automatically
do-nothing = Do nothing

## Markers window
clear-markers = { clear } { markers }
clear-spent-autoplace = Reset spent auto-placement

## Edit markers window

edit-markers = Create/edit markers
set-map-id = Set Map ID to current map
current-squad-markers = current squad markers
take-squad-markers = Take from { current-squad-markers }
cannot-take-squad-markers = Cannot take from { current-squad-markers }; not in a squad.
rt-api-required-squad-markers = { rt-api-required-base } taking squad marker locations automatically.
no-position = No position provided.
trigger = Trigger: { $position }
position-plain = { $position }
position-get = Get current { position }
set-manually = Set manually
manual-position = Manual { position }
set-manually-save = { save } manual { position }
trigger-explanation = A trigger for a marker set is a 15m radius sphere with its centre at the trigger location.

## Timer tab

reload-timers = Reload { timers }
timer-tab = { timer-window }
source-arg = Source: { $source }
source-adhoc = Source: Ad-hoc
select-a-timer = Please select a timer to configure!

## Timer window

no-phases-active = No phases currently active, no timers running.
reset-timers = { reset } { timers }

## Pathing

pathing = Pathing
trail = Trail
poi = POI
space = KatRender
reload-packs = Reload
unload-packs = Unload All
filter-options = Filter Options
searchbar-clear = Clear the search bar and results.
show-filter = Show filter options
hide-filter = Hide filter options
current-map = Current map
ignore-root = Ignore root state
ignore-leaf = Ignore leaf state
ignore-branch = Ignore branch state
show-hidden = Show hidden
show-all = Show all
#off-map = Elsewhere
ignore-whitespace = Ignore spaces
case-insensitive = Ignore case
toggle = Toggle
pathing-config = Pathing Options
pathing-config-enable = {space} Pathing (Experimental)
pathing-config-minimap = Minimap Options
pathing-config-worldmap = Map Options
pathing-config-trail-alpha = Opacity
pathing-config-trail-alpha-minimap = Minimap Opacity
pathing-config-trail-alpha-worldmap = Map Opacity
pathing-config-poi-alpha = Billboard Opacity
pathing-config-poi-alpha-minimap = POI Minimap Opacity
pathing-config-poi-alpha-worldmap = POI Opacity
pathing-config-trail-scale = Scale
pathing-config-trail-scale-minimap = Minimap Scale
pathing-config-trail-scale-worldmap = Map Scale
pathing-config-poi-scale = Billboard Size
pathing-config-poi-scale-minimap = POI Minimap Size
pathing-config-poi-scale-worldmap = POI Size
pathing-config-player-overlap-threshold = Fade near player
pathing-config-distance-fade-intensity = Intensity
pathing-config-distance-max = Distance
pathing-config-textured = Textured trails
pathing-config-textured-minimap = Textured trails
pathing-config-textured-worldmap = Textured trails
pathing-config-map-open = Fwoom
pathing-config-camera-source = Camera Data Source
pathing-config-advanced = Advanced Settings
pathing-config-trail-notice = Trail generation settings may require a map change or reload to take effect, and may not work as you might expect.
pathing-config-trail-y-offset = Vertical Offset
pathing-config-trail-resolution = Trail Resolution
pathing-config-trail-width = Base Width
pathing-config-goggles = X-ray Goggles Experiment
pathing-config-goggles-notice = This currently requires setting Render Sampling to Native under Graphics Options.
pathing-config-festivals = {festival}s
pathing-config-festival-active = {$festival} (active)
pathing-config-reset-notice = Right-click any slider below to restore its default setting.
pathing-config-edge-feather-scale = edge feather scale
pathing-config-corner-boundary-scale = corner boundary scale
pathing-notice-space = {space} is required for pathing functionality.
pathing-notice-mumblelink = if you experience stuttering, try changing Vertical Sync under the in-game graphical settings
pathing-notice-rtapi-missing = RTAPI is a separate addon that must be installed via Nexus
pathing-notice-rtapi = if you experience stuttering, try changing Vertical Sync or switching to MumbleLink
mumblelink = MumbleLink
rtapi = Nexus RealTime API

## Space

render-unload = Unload Render
render-reload = Reload Render
render-notice-gameplay = Load in to the game to get started
render-notice-gameplay-initial = Select a character to get started
render-notice-error = Error! See log in Nexus or Taimi addon folder for more details
packs-empty = No files loaded
packs-empty-notice = Once installed from the { data-sources-tab } tab or downloaded manually, the "Reload" button should pick them up!

## Festivals

festival = Festival
halloween = Halloween
wintersday = Wintersday
superadventurefestival = Super Adventure Box
lunarnewyear = Lunar New Year
festivalofthefourwinds = Festival Of The Four Winds
dragonbash = Dragon Bash

## Gamebinds (see `default_keybind`s in src/exports/runtime/bindings/controls.rs)
Miscellaneous_Interact = Interact
UI_ShowHideUI = Hide UI
Map_OpenClose = Map Open
Map_ZoomIn = Map Zoom +
Map_ZoomOut = Map Zoom -
Map_FloorUp = Map Floor +
Map_FloorDown = Map Floor -
Map_Recenter = Map Recenter
gamebind-marker-arrow = Arrow
gamebind-marker-circle = Circle
gamebind-marker-heart = Heart
gamebind-marker-square = Square
gamebind-marker-star = Star
gamebind-marker-spiral = Spiral
gamebind-marker-triangle = Triangle
gamebind-marker-x = X
gamebind-marker-clear = Clear Markers
gamebind-marker-location-suffix = {""}
gamebind-marker-object-suffix = {" "}(Target)
# common
Squad_Location_Arrow = {gamebind-marker-arrow}{gamebind-marker-location-suffix}
Squad_Location_Circle = {gamebind-marker-circle}{gamebind-marker-location-suffix}
Squad_Location_Heart = {gamebind-marker-heart}{gamebind-marker-location-suffix}
Squad_Location_Square = {gamebind-marker-square}{gamebind-marker-location-suffix}
Squad_Location_Star = {gamebind-marker-star}{gamebind-marker-location-suffix}
Squad_Location_Spiral = {gamebind-marker-spiral}{gamebind-marker-location-suffix}
Squad_Location_Triangle = {gamebind-marker-triangle}{gamebind-marker-location-suffix}
Squad_Location_X = {gamebind-marker-x}{gamebind-marker-location-suffix}
Squad_Object_Arrow = {gamebind-marker-arrow}{gamebind-marker-object-suffix}
Squad_Object_Circle = {gamebind-marker-circle}{gamebind-marker-object-suffix}
Squad_Object_Heart = {gamebind-marker-heart}{gamebind-marker-object-suffix}
Squad_Object_Square = {gamebind-marker-square}{gamebind-marker-object-suffix}
Squad_Object_Star = {gamebind-marker-star}{gamebind-marker-object-suffix}
Squad_Object_Spiral = {gamebind-marker-spiral}{gamebind-marker-object-suffix}
Squad_Object_Triangle = {gamebind-marker-triangle}{gamebind-marker-object-suffix}
Squad_Object_X = {gamebind-marker-x}{gamebind-marker-object-suffix}
Squad_ClearAllLocationMarkers = {gamebind-marker-clear}{gamebind-marker-location-suffix}
Squad_ClearAllObjectMarkers = {gamebind-marker-clear}{gamebind-marker-object-suffix}
