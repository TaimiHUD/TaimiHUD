## Common

height = Height
font = Font
okay = OK
delete = Delete
save = Save
delete-item = { delete } "{ $item }"?
save-item = { save } "{ $item }"?
save-standalone = { save } as a new file
save-append = Append to an existing file
save-edit = { save } edited changes
save-edit-item = { save-edit } to "{ $item }"?
save-mode = Save mode
error = Error
unknown = Unknown
update = Update
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
enable = Enable
cancel = Cancel
disable = Disable
enabled = { enable }d
disabled = { disable }d
author-arg = { author }: { $author }
reset = Reset
reset-timers = { reset } { timers }
timer = Timer
timers = { timer }s
experimental-notice = Hi! This feature is (mostly) experimental. Some things may be confusing and it might require more thought and effort to use than the less experimental features. My apologies to any problems you have; feel free to reach out on Discord. - Kat
name-empty = Name empty.
no-trigger = No trigger position provided.
no-category = No category provided.
map-id-wrong = Map ID incorrect.
no-positions = No marker positions provided.
validation-fail = Validation failed due to:
filename-empty = No filename provided.
count = Count
actions = Actions

## Addon

primary-window-toggle = Taimi Window Toggle
timer-window-toggle = Timer Window Toggle
marker-window-toggle = Marker Window Toggle
pathing-window-toggle = Pathing Window Toggle
primary-window-toggle-text = Show/hide taimi primary window
timer-key-trigger = Timer Key Trigger { $id }

## Config

config-tab = Config
stock-imgui-progress-bar = Stock Imgui Progress Bar
shadow = Shadow
centre-text-after-icon = Centre text after icon
imgui-notice = You can control-click on a slider element, or such, to be able to directly input data to it. Remember to press enter after inputting the value.
marker-trigger = Marker set position trigger behaviour
marker-condition = Behaviour condition
autoplace-warning = If you do not have RTAPI installed, we will not be able to detect whether you are a lieutenant instead of just a commander.

## Windows

primary-window = TaimiHUD
timer-window = Encounter Timers
marker-window = Squad Markers
pathing-window = Pathing

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

data-sources-tab = Data Sources
checking-for-updates = Checking for updates!
check-for-updates = Check for updates
check-for-updates-tooltip = Check for updates to any data sources. We don't do this automatically to respect your choice on whether or not to make network requests.
checked-for-updates-last = Last checked for updates at: { $time }
reload-data-sources = Reload data sources
reload-data-sources-tooltip = Reload items from currently installed data sources. Useful if you have changed the files within them!

remote = Remote
update-status = Update Status
actions = Actions
version-installed = Installed version: { $version }
version-not-installed = Not installed
update-unknown = Update status unknown; check for updates?
update-not-required = Update not required; up to date!
update-available = New version available: { $version }!
update-error = { error } updating: { $error }!
attempt-update = Attempt to update anyway?
settings-unloaded = Settings have not yet loaded!

## Info tab

info-tab = Info
keybind-triggers = If you need keybind-based timer triggers, please bind the appropriate keys in the Nexus settings.
active-timer-phases = Active timer phases
timer = Timer
phase = Phase
# As in, like, "game engine" or "rendering engine" :o
engine = Engine
ecs-data = ECS { data }
object-data = { object } { data }
object-kind = { object } Kind
model-files = { model } Files
vertices = Vertices
textures = Textures: { $count }

## Markers tab

reload-markers = Reload { markers }
marker-tab = { marker-window }
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
