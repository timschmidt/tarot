from pathlib import Path
from html import escape
import json, shutil, math

# LG Fx0 composition: preserve the 360-unit design density on a shorter display.
W, H, SCALE = 360, 640, 2
STATUS_H, GESTURE_H = 32, 36
GESTURE_Y = H - GESTURE_H
APP_H = GESTURE_Y - STATUS_H
BANNER_H, DASHBOARD_H, KEYBOARD_H = 32, 183, 220
KEYBOARD_Y = GESTURE_Y - KEYBOARD_H
BOTTOM_SHIFT = H - 720
CARD_Y = 112

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT
SCREENS = OUT / 'screens' / 'svg'
for p in [SCREENS, OUT/'screens'/'png', OUT/'brand', OUT/'wallpapers', OUT/'tokens', OUT/'fonts', OUT/'source', OUT/'components', OUT/'icons', OUT/'glyphs']:
    p.mkdir(parents=True, exist_ok=True)
# Bundled font files are already present in fonts/.

THEMES = {
    'fx0_gold': {'name':'Fx0 Gold','top':'#211B12','mid':'#51432B','bottom':'#8D7646','glow':'#C8B586','line':'#E2CF9F','accent':'#795A27','focus':'#E3C581'},
    'nocturne': {'name':'Nocturne','top':'#091A21','mid':'#173D43','bottom':'#31534D','glow':'#708B79','line':'#C6AE78','accent':'#A3763F','focus':'#D7BE89'},
    'heritage': {'name':'Heritage','top':'#1D3027','mid':'#4A6650','bottom':'#9A9451','glow':'#CBC883','line':'#E8DCAD','accent':'#3C7C96','focus':'#A9D9E6'},
    'vellum': {'name':'Vellum','top':'#34242C','mid':'#65454A','bottom':'#926653','glow':'#C3A477','line':'#EAD3A1','accent':'#934C4C','focus':'#E0BF95'},
}

DEFAULT_THEME = 'fx0_gold'

# Component roles preserve the legacy reference palettes, while the Gold override
# changes only shell-owned chrome and cooperative sample app chrome.
BASE_UI_ROLES = {
    "status.background": "#030505",
    "status.title": "#F6F7F4",
    "status.menu_arrow": "#BEC6C2",
    "status.clock": "#FFFFFF",
    "status.network_label": "#EBF0E8",
    "status.network_glyph": "#EDF4E7",
    "status.battery_border": "#C3CCC2",
    "status.battery_charge": "#9BCB76",
    "status.battery_cap": "#BCC8B9",
    "gesture.background": "#070D10",
    "gesture.edge": "#374345",
    "gesture.active_indicator": "#B9D3D2",
    "gesture.idle_indicator": "#536C70",
    "frame_corners.fill": "#030505",
    "just_type.field": "#0A1519",
    "just_type.border": "#BDCCC3",
    "just_type.placeholder": "#DAE1D8",
    "just_type.glyph": "#D4DDD3",
    "dock.border": "#C5CFC5",
    "dock.highlight": "#E5EADF",
    "dock.wave_border": "#E1E7D9",
    "dock.wave_label": "#F7F4E6",
    "app_header.border": "#9BA39B",
    "app_header.highlight": "#F7F8F1",
    "app_header.title": "#303936",
    "app_header.action": "#5B777E",
    "command_bar.border": "#9FA69F",
    "command_bar.control_border": "#BCC5BA",
    "command_bar.glyph": "#DDE7D8",
    "command_bar.separator": "#151C1C",
    "email_content.surface": "#E5E8DF",
    "email_content.search_surface": "#D2D7CC",
    "email_content.search_border": "#ADB6AC",
    "email_content.search_glyph": "#6D7D78",
    "email_content.search_placeholder": "#4E5F48",
    "email_content.section_surface": "#BCC5B6",
    "email_content.section_label": "#536B60",
    "email_content.divider": "#C4CCC0",
    "email_content.unread_dot": "#628CA0",
    "email_content.primary_text": "#2D3933",
    "email_content.metadata": "#58674F",
    "email_content.subject": "#435247",
    "email_content.preview": "#58674F",
    "calendar_content.surface": "#F1F0E7",
    "calendar_content.month": "#46544B",
    "calendar_content.navigation": "#738177",
    "calendar_content.hours": "#5D6C55",
    "calendar_content.grid": "#D0D4C8",
    "calendar_content.event_a_surface": "#D3E7DF",
    "calendar_content.event_a_border": "#97B6AD",
    "calendar_content.event_a_rail": "#6F9F91",
    "calendar_content.event_a_title": "#405D54",
    "calendar_content.event_a_metadata": "#688377",
    "calendar_content.event_b_surface": "#E7DFC2",
    "calendar_content.event_b_border": "#BAAE83",
    "calendar_content.event_b_rail": "#AB965D",
    "calendar_content.event_b_title": "#645C3B",
    "calendar_content.event_b_metadata": "#857A50",
    "calendar_content.toolbar_border": "#9FA69F",
    "calendar_content.control_border": "#BCC5BA",
    "calendar_content.active_tab": "#EFF2E7",
    "calendar_content.inactive_tab": "#B8C5B0",
    "calendar_content.separator": "#172122",
    "message_content.surface": "#DADFD2",
    "message_content.date": "#5D6C55",
    "message_content.received_surface": "#F4F5EB",
    "message_content.sent_surface": "#C4D6B4",
    "message_content.bubble_border": "#AEB9A5",
    "message_content.message_text": "#40513D",
    "message_content.compose_border": "#A7B39E",
    "message_content.compose_placeholder": "#86927E",
    "message_content.compose_glyph": "#6B8171",
    "web_content.surface": "#F6F4EB",
    "web_content.address_surface": "#E3E6DB",
    "web_content.address_border": "#B7C0B0",
    "web_content.address_text": "#7C8778",
    "web_content.heading": "#394D45",
    "web_content.body_text": "#6C7B6A",
    "group_box.border": "#A7AFA4",
    "group_box.heading_surface": "#ACB6A5",
    "group_box.heading_label": "#F8FAF2",
    "group_box.divider": "#CCD2C5",
    "group_box.row_label": "#34432F",
    "group_box.switch_off_surface": "#A4ADA0",
    "group_box.switch_border": "#7E8D7A",
    "group_box.switch_knob_border": "#B1BAAB",
    "group_box.switch_label": "#F9FFF3",
    "group_box.row_value": "#5D6C55",
    "group_box.chevron": "#9AA891",
    "settings_content.surface": "#DFE3D8",
    "settings_content.border": "#A7AFA4",
    "settings_content.heading_surface": "#ACB6A5",
    "settings_content.heading_label": "#F6F8EF",
    "settings_content.row_label": "#394835",
    "settings_content.slider_track": "#A1B5B6",
    "settings_content.slider_fill": "#76A8BD",
    "settings_content.slider_knob_border": "#789AA6",
    "settings_content.slider_knob_center": "#A6CDDC",
    "card.backing": "#E9EDE3",
    "card.border": "#DAE3D7",
    "launcher.scrim": "#102327",
    "launcher.page_label": "#F2F4EA",
    "launcher.previous_page": "#63756C",
    "launcher.next_page": "#BFCABE",
    "launcher.app_label": "#F3F4E9",
    "launcher.scroll_cue": "#A8BBAA",
    "full_app.banner_surface": "#101719",
    "full_app.banner_text": "#EFF3E8",
    "full_app.dashboard_surface": "#050B0C",
    "full_app.dashboard_title": "#FAF9F1",
    "full_app.dashboard_secondary": "#CBD3C7",
    "full_app.badge_surface": "#8DABBD",
    "full_app.badge_border": "#E0E7DD",
    "full_app.wave_scrim": "#071015",
    "full_app.overlay": "#000",
    "full_app.device_menu_border": "#88958C",
    "full_app.menu_heading": "#E4EBE0",
    "full_app.menu_secondary": "#C5D0C0",
    "full_app.menu_separator": "#111A1B",
    "full_app.menu_glyph": "#D4DFCD",
    "full_app.menu_label": "#F4F6EF",
    "full_app.menu_value": "#ADBFA5",
    "full_app.menu_row_separator": "#172122",
    "full_app.slider_glyph": "#D5DDCD",
    "full_app.slider_track": "#141E20",
    "full_app.slider_fill": "#9AAB91",
    "full_app.slider_border": "#9AA890",
    "full_app.preferences_label": "#D3DED0",
    "full_app.app_menu_border": "#89968D",
    "full_app.app_menu_label": "#F2F5EA",
    "full_app.submenu_glyph": "#BECBB6",
    "full_app.app_menu_separator": "#131D1E",
    "keyboard.surface": "#9FA8A2",
    "keyboard.edge": "#E5EBDF",
    "keyboard.key_border": "#737F73",
    "keyboard.key_label": "#303D30",
    "keyboard.shift_label": "#F7F7ED",
    "keyboard.back_glyph": "#F2F6E9",
    "keyboard.bottom_key_border": "#74816F",
    "keyboard.go_label": "#F5F7ED",
    "keyboard.bottom_key_label": "#59634F",
    "search_screen.surface": "#E6EADF",
    "search_screen.field": "#F6F8EE",
    "search_screen.field_border": "#A7B2A1",
    "search_screen.glyph": "#779174",
    "search_screen.query": "#34462E",
    "search_screen.caret": "#5C8172",
    "search_screen.clear_glyph": "#9BAC98",
    "search_screen.section_surface": "#C1CCB8",
    "search_screen.section_label": "#5F7654",
    "search_screen.result_title": "#354C2E",
    "search_screen.result_subtitle": "#6B8362",
    "search_screen.result_metadata": "#5D6C55",
    "search_screen.action": "#49673E",
    "search_screen.scroll_cue": "#879B7D",
    "notification_rail.surface": "#0B1113",
    "launch_pending.border": "#7B9187",
    "launch_pending.title": "#EFF2E5",
    "launch_pending.secondary": "#BDCEB5",
    "launch_pending.retry_border": "#8A9C83",
    "launch_pending.retry_label": "#45583D",
    "launch_pending.active_dot": "#D1DFC7",
    "launch_pending.idle_dot": "#687E66",
    "lock_screen.scrim": "#010B10",
    "lock_screen.clock": "#F5F4E8",
    "lock_screen.date": "#D4DED0",
    "lock_screen.notification_surface": "#122629",
    "lock_screen.notification_border": "#536C66",
    "lock_screen.notification_title": "#ECF1E5",
    "lock_screen.notification_secondary": "#B9CAB8",
    "lock_screen.target_surface": "#101E23",
    "lock_screen.target_border": "#CFD8C5",
    "lock_screen.handle_border": "#C3CFC2",
    "lock_screen.lock_glyph": "#E4EADB",
    "lock_screen.instruction": "#BFCDBC",
    "lock_screen.pin_title": "#EDF1E4",
    "lock_screen.pin_indicator": "#D8E1D0",
    "lock_screen.key_border": "#AABBA8",
    "lock_screen.key_label": "#F1F1E3",
    "lock_screen.cancel_label": "#CFDCC9",
    "lock_screen.back_glyph": "#D7E1D0",
    "lock_screen.emergency_label": "#D4E0CC",
    "incoming.clock": "#F4F3E7",
    "incoming.panel_border": "#747E74",
    "incoming.panel_footer": "#202527",
    "incoming.caller": "#F4F5E9",
    "incoming.caller_type": "#C0CEB8",
    "incoming.state": "#A7BA9F",
    "incoming.decline_border": "#B3988D",
    "incoming.decline_label": "#F9ECE6",
    "incoming.answer_border": "#B7C19C",
    "incoming.answer_label": "#F4F8DF",
    "incoming_locked.scrim": "#061117",
    "incoming_locked.caller": "#F1F1E4",
    "incoming_locked.secondary": "#B7CCB6",
    "incoming_locked.target_surface": "#152C2D",
    "incoming_locked.target_border": "#6A8577",
    "incoming_locked.handle_border": "#BCCBB5",
    "incoming_locked.instruction": "#C4D5BE",
    "ongoing_call.surface": "#172D28",
    "ongoing_call.caller": "#E6EDDC",
    "ongoing_call.caller_type": "#9FB797",
    "ongoing_call.avatar_surface": "#647C6B",
    "ongoing_call.avatar_figure": "#B3C1A9",
    "ongoing_call.timer": "#DDE8D4",
    "ongoing_call.control_border": "#73836E",
    "ongoing_call.control_glyph": "#DFE8D1",
    "ongoing_call.control_label": "#BBCBB0",
    "ongoing_call.end_border": "#A69783",
    "ongoing_call.end_label": "#FAEDE3",
    "boot.scrim": "#06121A",
    "boot.mark": "#CDB98B",
    "boot.wordmark": "#F0E9D4",
    "brand_mark.field": "#0B232A",
    "brand.stroke": "#D0BA87",
    "brand.wordmark": "#173A40",
    "settings.value_accent": "#537B8D",
    "switch.off_label": "#F9FFF3",
    "switch.on_label": "#F9FFF3",
    "focus.on_dark": "#D7BE89",
    "focus.on_light": "#537B8D",
    "gradient.chrome.start": "#F0F0EB",
    "gradient.chrome.middle": "#D6D8D1",
    "gradient.chrome.end": "#B7BCB5",
    "gradient.surface.start": "#F2F3ED",
    "gradient.surface.end": "#DEE1D8",
    "gradient.row.start": "#FAFAF5",
    "gradient.row.end": "#E5E7DE",
    "gradient.dark.start": "#484C4D",
    "gradient.dark.middle": "#303536",
    "gradient.dark.end": "#202527",
    "gradient.menu.start": "#484D4C",
    "gradient.menu.end": "#262D2E",
    "gradient.button.start": "#FAFBF8",
    "gradient.button.high": "#DEE1DA",
    "gradient.button.low": "#D2D6CE",
    "gradient.button.end": "#C0C6BC",
    "gradient.accent.start": "#7AC6EC",
    "gradient.accent.middle": "#388FB8",
    "gradient.accent.end": "#1E536F",
    "gradient.success_action.start": "#B4D368",
    "gradient.success_action.middle": "#70A849",
    "gradient.success_action.end": "#34733D",
    "gradient.destructive_action.start": "#E5816D",
    "gradient.destructive_action.middle": "#CB4D40",
    "gradient.destructive_action.end": "#9B302C",
    "gradient.halo.start": "#FFF1BF",
    "gradient.halo.end": "#FFDC81",
    "gradient.tray.start": "#FFFFFF",
    "gradient.tray.middle": "#999E99",
    "gradient.tray.end": "#171D20",
    "components.surface": "#E5E8DF",
    "components.label": "#596B53",
    "components.pressed_surface": "#31534D",
    "components.disabled_surface": "#D2D7CA",
    "components.border": "#8B9B81",
    "components.pressed_label": "#F3F6EA",
    "components.disabled_label": "#98A28E",
    "components.button_label": "#3F5137",
    "components.switch_off_surface": "#AAB39F",
    "components.switch_border": "#7D8F73",
    "components.disabled_knob": "#C4CBBC",
    "components.knob_border": "#A5B09C",
    "components.switch_label": "#F3F5E9",
    "components.pending_indicator": "#486C5D",
    "glyph.default": "#E4EAE0"
}
GOLD_UI_OVERRIDES = {
    "status.background": "#030505",
    "status.title": "#FFF8E8",
    "status.menu_arrow": "#CCBC98",
    "status.clock": "#FFF8E8",
    "status.network_label": "#F3ECDD",
    "status.network_glyph": "#F3ECDD",
    "status.battery_border": "#D3C5A7",
    "status.battery_charge": "#9BCB76",
    "status.battery_cap": "#CCBC98",
    "gesture.background": "#17120C",
    "gesture.edge": "#665333",
    "gesture.active_indicator": "#E3C581",
    "gesture.idle_indicator": "#8F774D",
    "frame_corners.fill": "#030505",
    "just_type.field": "#211B12",
    "just_type.border": "#DAC79D",
    "just_type.placeholder": "#F3ECDD",
    "just_type.glyph": "#E2CF9F",
    "dock.border": "#D6C49C",
    "dock.highlight": "#F8EACB",
    "dock.wave_border": "#EAD6A6",
    "dock.wave_label": "#FFF8E8",
    "app_header.border": "#AF9C76",
    "app_header.highlight": "#FFF9ED",
    "app_header.title": "#332A1C",
    "app_header.action": "#59482E",
    "command_bar.border": "#AF9C76",
    "command_bar.control_border": "#BFAE8A",
    "command_bar.glyph": "#FFF8E8",
    "command_bar.separator": "#18140D",
    "email_content.surface": "#F3ECDD",
    "email_content.search_surface": "#E2D7BF",
    "email_content.search_border": "#BAA986",
    "email_content.search_glyph": "#795A27",
    "email_content.search_placeholder": "#59482E",
    "email_content.section_surface": "#D0BD95",
    "email_content.section_label": "#332A1C",
    "email_content.divider": "#CBBDA0",
    "email_content.unread_dot": "#795A27",
    "email_content.primary_text": "#332A1C",
    "email_content.metadata": "#59482E",
    "email_content.subject": "#443720",
    "email_content.preview": "#59482E",
    "calendar_content.surface": "#F3ECDD",
    "calendar_content.month": "#332A1C",
    "calendar_content.navigation": "#795A27",
    "calendar_content.hours": "#59482E",
    "calendar_content.grid": "#D7CAB0",
    "calendar_content.event_a_surface": "#E8D9B7",
    "calendar_content.event_a_border": "#BCA06C",
    "calendar_content.event_a_rail": "#9B783D",
    "calendar_content.event_a_title": "#493817",
    "calendar_content.event_a_metadata": "#59482E",
    "calendar_content.event_b_surface": "#E4D3BF",
    "calendar_content.event_b_border": "#B79B7F",
    "calendar_content.event_b_rail": "#8D6447",
    "calendar_content.event_b_title": "#4F3825",
    "calendar_content.event_b_metadata": "#59482E",
    "calendar_content.toolbar_border": "#AF9C76",
    "calendar_content.control_border": "#BFAE8A",
    "calendar_content.active_tab": "#FFF8E8",
    "calendar_content.inactive_tab": "#DDCEAF",
    "calendar_content.separator": "#18140D",
    "message_content.surface": "#E9E0CB",
    "message_content.date": "#59482E",
    "message_content.received_surface": "#FBF5E8",
    "message_content.sent_surface": "#DBC797",
    "message_content.bubble_border": "#BDAA83",
    "message_content.message_text": "#332A1C",
    "message_content.compose_border": "#B7A47B",
    "message_content.compose_placeholder": "#59482E",
    "message_content.compose_glyph": "#795A27",
    "web_content.surface": "#F3ECDD",
    "web_content.address_surface": "#E5DCC8",
    "web_content.address_border": "#C2B28F",
    "web_content.address_text": "#59482E",
    "web_content.heading": "#332A1C",
    "web_content.body_text": "#59482E",
    "group_box.border": "#B8A783",
    "group_box.heading_surface": "#D0BD95",
    "group_box.heading_label": "#332A1C",
    "group_box.divider": "#D1C4A8",
    "group_box.row_label": "#332A1C",
    "group_box.switch_off_surface": "#D0CBBF",
    "group_box.switch_border": "#9C8C6D",
    "group_box.switch_knob_border": "#B5A483",
    "group_box.switch_label": "#FFF8E8",
    "group_box.row_value": "#59482E",
    "group_box.chevron": "#795A27",
    "settings_content.surface": "#F3ECDD",
    "settings_content.border": "#B8A783",
    "settings_content.heading_surface": "#D0BD95",
    "settings_content.heading_label": "#332A1C",
    "settings_content.row_label": "#332A1C",
    "settings_content.slider_track": "#C5B99F",
    "settings_content.slider_fill": "#795A27",
    "settings_content.slider_knob_border": "#8E713A",
    "settings_content.slider_knob_center": "#B09256",
    "card.backing": "#F3ECDD",
    "card.border": "#D7C8A8",
    "launcher.scrim": "#211B12",
    "launcher.page_label": "#FFF8E8",
    "launcher.previous_page": "#B3A17D",
    "launcher.next_page": "#E2CF9F",
    "launcher.app_label": "#FFF8E8",
    "launcher.scroll_cue": "#E2CF9F",
    "full_app.banner_surface": "#211B12",
    "full_app.banner_text": "#FFF8E8",
    "full_app.dashboard_surface": "#130F09",
    "full_app.dashboard_title": "#FFF8E8",
    "full_app.dashboard_secondary": "#E0D0AE",
    "full_app.badge_surface": "#795A27",
    "full_app.badge_border": "#E3C581",
    "full_app.wave_scrim": "#211B12",
    "full_app.overlay": "#000000",
    "full_app.device_menu_border": "#9E8A62",
    "full_app.menu_heading": "#FFF8E8",
    "full_app.menu_secondary": "#E0D0AE",
    "full_app.menu_separator": "#17120C",
    "full_app.menu_glyph": "#F3ECDD",
    "full_app.menu_label": "#FFF8E8",
    "full_app.menu_value": "#DFCEAB",
    "full_app.menu_row_separator": "#17120C",
    "full_app.slider_glyph": "#F3ECDD",
    "full_app.slider_track": "#17120C",
    "full_app.slider_fill": "#E3C581",
    "full_app.slider_border": "#C3AA73",
    "full_app.preferences_label": "#F3ECDD",
    "full_app.app_menu_border": "#9E8A62",
    "full_app.app_menu_label": "#FFF8E8",
    "full_app.submenu_glyph": "#E2CF9F",
    "full_app.app_menu_separator": "#17120C",
    "keyboard.surface": "#B5AA92",
    "keyboard.edge": "#F7EFDC",
    "keyboard.key_border": "#8F7F60",
    "keyboard.key_label": "#332A1C",
    "keyboard.shift_label": "#FFF8E8",
    "keyboard.back_glyph": "#FFF8E8",
    "keyboard.bottom_key_border": "#8F7F60",
    "keyboard.go_label": "#FFF8E8",
    "keyboard.bottom_key_label": "#332A1C",
    "search_screen.surface": "#F3ECDD",
    "search_screen.field": "#FBF5E8",
    "search_screen.field_border": "#BCAA85",
    "search_screen.glyph": "#795A27",
    "search_screen.query": "#332A1C",
    "search_screen.caret": "#795A27",
    "search_screen.clear_glyph": "#795A27",
    "search_screen.section_surface": "#D0BD95",
    "search_screen.section_label": "#332A1C",
    "search_screen.result_title": "#332A1C",
    "search_screen.result_subtitle": "#59482E",
    "search_screen.result_metadata": "#59482E",
    "search_screen.action": "#795A27",
    "search_screen.scroll_cue": "#795A27",
    "notification_rail.surface": "#17120C",
    "launch_pending.border": "#AB9362",
    "launch_pending.title": "#FFF8E8",
    "launch_pending.secondary": "#E0D0AE",
    "launch_pending.retry_border": "#B29B70",
    "launch_pending.retry_label": "#332A1C",
    "launch_pending.active_dot": "#E3C581",
    "launch_pending.idle_dot": "#8A7650",
    "lock_screen.scrim": "#211B12",
    "lock_screen.clock": "#FFF8E8",
    "lock_screen.date": "#F3ECDD",
    "lock_screen.notification_surface": "#302719",
    "lock_screen.notification_border": "#8B754F",
    "lock_screen.notification_title": "#FFF8E8",
    "lock_screen.notification_secondary": "#E2D4B6",
    "lock_screen.target_surface": "#302719",
    "lock_screen.target_border": "#E3C581",
    "lock_screen.handle_border": "#D2BC89",
    "lock_screen.lock_glyph": "#FFF8E8",
    "lock_screen.instruction": "#F3ECDD",
    "lock_screen.pin_title": "#FFF8E8",
    "lock_screen.pin_indicator": "#F3ECDD",
    "lock_screen.key_border": "#D2BC89",
    "lock_screen.key_label": "#FFF8E8",
    "lock_screen.cancel_label": "#F3ECDD",
    "lock_screen.back_glyph": "#F3ECDD",
    "lock_screen.emergency_label": "#F3ECDD",
    "incoming.clock": "#FFF8E8",
    "incoming.panel_border": "#8D7952",
    "incoming.panel_footer": "#242016",
    "incoming.caller": "#FFF8E8",
    "incoming.caller_type": "#E0D0AE",
    "incoming.state": "#E0D0AE",
    "incoming.decline_border": "#C78F79",
    "incoming.decline_label": "#FFF8E8",
    "incoming.answer_border": "#94B27A",
    "incoming.answer_label": "#FFF8E8",
    "incoming_locked.scrim": "#211B12",
    "incoming_locked.caller": "#FFF8E8",
    "incoming_locked.secondary": "#F3ECDD",
    "incoming_locked.target_surface": "#302719",
    "incoming_locked.target_border": "#8D7952",
    "incoming_locked.handle_border": "#D2BC89",
    "incoming_locked.instruction": "#F3ECDD",
    "ongoing_call.surface": "#302719",
    "ongoing_call.caller": "#FFF8E8",
    "ongoing_call.caller_type": "#E0D0AE",
    "ongoing_call.avatar_surface": "#776448",
    "ongoing_call.avatar_figure": "#C9B68C",
    "ongoing_call.timer": "#FFF8E8",
    "ongoing_call.control_border": "#9A8358",
    "ongoing_call.control_glyph": "#FFF8E8",
    "ongoing_call.control_label": "#E0D0AE",
    "ongoing_call.end_border": "#C78F79",
    "ongoing_call.end_label": "#FFF8E8",
    "boot.scrim": "#211B12",
    "boot.mark": "#E3C581",
    "boot.wordmark": "#FFF8E8",
    "brand_mark.field": "#211B12",
    "brand.stroke": "#E3C581",
    "brand.wordmark": "#795A27",
    "settings.value_accent": "#795A27",
    "switch.off_label": "#332A1C",
    "switch.on_label": "#FFF8E8",
    "focus.on_dark": "#E3C581",
    "focus.on_light": "#795A27",
    "gradient.chrome.start": "#FBF5E8",
    "gradient.chrome.middle": "#E5D7B9",
    "gradient.chrome.end": "#D0BD95",
    "gradient.surface.start": "#F8F2E5",
    "gradient.surface.end": "#E7DCC3",
    "gradient.row.start": "#FBF5E8",
    "gradient.row.end": "#EEE4D0",
    "gradient.dark.start": "#49402E",
    "gradient.dark.middle": "#352E20",
    "gradient.dark.end": "#242016",
    "gradient.menu.start": "#49402E",
    "gradient.menu.end": "#242016",
    "gradient.button.start": "#FFF9ED",
    "gradient.button.high": "#E8DCC3",
    "gradient.button.low": "#DFD1B3",
    "gradient.button.end": "#CDBA93",
    "gradient.accent.start": "#886326",
    "gradient.accent.middle": "#735322",
    "gradient.accent.end": "#5E451D",
    "gradient.success_action.start": "#43793A",
    "gradient.success_action.middle": "#386A32",
    "gradient.success_action.end": "#2D592A",
    "gradient.destructive_action.start": "#AC4538",
    "gradient.destructive_action.middle": "#93372E",
    "gradient.destructive_action.end": "#7E2B25",
    "gradient.halo.start": "#F3DDA8",
    "gradient.halo.end": "#CBA45A",
    "gradient.tray.start": "#FFF7E1",
    "gradient.tray.middle": "#B7A27B",
    "gradient.tray.end": "#2B2418",
    "components.surface": "#F3ECDD",
    "components.label": "#59482E",
    "components.pressed_surface": "#795A27",
    "components.disabled_surface": "#DED8CA",
    "components.border": "#A28B5D",
    "components.pressed_label": "#FFF8E8",
    "components.disabled_label": "#796A4F",
    "components.button_label": "#332A1C",
    "components.switch_off_surface": "#D0CBBF",
    "components.switch_border": "#9C8C6D",
    "components.disabled_knob": "#DCD4C2",
    "components.knob_border": "#B5A483",
    "components.switch_label": "#FFF8E8",
    "components.pending_indicator": "#795A27",
    "glyph.default": "#F3ECDD"
}
UI_ROLES = {name: dict(BASE_UI_ROLES, **(GOLD_UI_OVERRIDES if name == 'fx0_gold' else {})) for name in THEMES}

def text(x,y,s,size=16,fill='#242727',weight='normal',anchor='start',family='Lato',extra=''):
    return f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" font-weight="{weight}" fill="{fill}" text-anchor="{anchor}" {extra}>{escape(str(s))}</text>'

def rect(x,y,w,h,fill,rx=0,stroke=None,sw=1,extra=''):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+f' {extra}/>'

def line(x1,y1,x2,y2,c='#b8bab4',sw=1,extra=''):
    return f'<path d="M{x1} {y1} L{x2} {y2}" stroke="{c}" stroke-width="{sw}" fill="none" {extra}/>'

def circle(x,y,r,fill,stroke=None,sw=1,extra=''):
    return f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+f' {extra}/>'

def ui_gradient_defs(theme):
    """Themeable UI materials, kept separate from the pictorial icon gradients."""
    p = UI_ROLES[theme]
    definitions = {
        'uiChrome': ('chrome', [('start', '0'), ('middle', '.45'), ('end', '1')]),
        'uiSurface': ('surface', [('start', '0'), ('end', '1')]),
        'uiRow': ('row', [('start', '0'), ('end', '1')]),
        'uiDark': ('dark', [('start', '0'), ('middle', '.45'), ('end', '1')]),
        'uiMenu': ('menu', [('start', '0'), ('end', '1')]),
        'uiButton': ('button', [('start', '0'), ('high', '.48'), ('low', '.5'), ('end', '1')]),
        'uiAccent': ('accent', [('start', '0'), ('middle', '.5'), ('end', '1')]),
        'uiSuccessAction': ('success_action', [('start', '0'), ('middle', '.5'), ('end', '1')]),
        'uiDestructiveAction': ('destructive_action', [('start', '0'), ('middle', '.5'), ('end', '1')]),
    }
    result = ''
    for ident, (role, stops) in definitions.items():
        result += f'<linearGradient id="{ident}" x1="0" y1="0" x2="0" y2="1">'
        for stop, offset in stops:
            color = p[f'gradient.{role}.{stop}']
            result += f'<stop offset="{offset}" stop-color="{color}"/>'
        result += '</linearGradient>'
    result += '<linearGradient id="uiTray" x1="0" y1="0" x2="0" y2="1">'
    for stop, offset, opacity in [('start', '0', '.32'), ('middle', '.48', '.21'), ('end', '1', '.60')]:
        color = p[f'gradient.tray.{stop}']
        result += f'<stop offset="{offset}" stop-color="{color}" stop-opacity="{opacity}"/>'
    result += '</linearGradient>'
    result += '<radialGradient id="uiHalo">'
    for stop, offset, opacity in [('start', '0', '.65'), ('end', '1', '0')]:
        color = p[f'gradient.halo.{stop}']
        result += f'<stop offset="{offset}" stop-color="{color}" stop-opacity="{opacity}"/>'
    return result + '</radialGradient>'

def defs(theme=DEFAULT_THEME):
    t=THEMES[theme]
    return f'''<defs>
    {ui_gradient_defs(theme)}
    <linearGradient id="wall" x1="0" y1="0" x2="1" y2="1"><stop stop-color="{t['top']}"/><stop offset=".57" stop-color="{t['mid']}"/><stop offset="1" stop-color="{t['bottom']}"/></linearGradient>
    <radialGradient id="glow"><stop stop-color="{t['glow']}" stop-opacity=".50"/><stop offset="1" stop-color="{t['glow']}" stop-opacity="0"/></radialGradient>
    <linearGradient id="chrome" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F0F0EB"/><stop offset=".45" stop-color="#D6D8D1"/><stop offset="1" stop-color="#B7BCB5"/></linearGradient>
    <linearGradient id="surface" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F2F3ED"/><stop offset="1" stop-color="#DEE1D8"/></linearGradient>
    <linearGradient id="row" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FAFAF5"/><stop offset="1" stop-color="#E5E7DE"/></linearGradient>
    <linearGradient id="tray" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FFFFFF" stop-opacity=".32"/><stop offset=".48" stop-color="#999E99" stop-opacity=".21"/><stop offset="1" stop-color="#171D20" stop-opacity=".60"/></linearGradient>
    <linearGradient id="dark" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#484C4D"/><stop offset=".45" stop-color="#303536"/><stop offset="1" stop-color="#202527"/></linearGradient>
    <linearGradient id="menu" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#484D4C"/><stop offset="1" stop-color="#262D2E"/></linearGradient>
    <linearGradient id="green" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#B4D368"/><stop offset=".50" stop-color="#70A849"/><stop offset="1" stop-color="#34733D"/></linearGradient>
    <linearGradient id="blue" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#7AC6EC"/><stop offset=".50" stop-color="#388FB8"/><stop offset="1" stop-color="#1E536F"/></linearGradient>
    <linearGradient id="gold" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F6D985"/><stop offset=".5" stop-color="#C7A052"/><stop offset="1" stop-color="#92713F"/></linearGradient>
    <linearGradient id="red" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#E5816D"/><stop offset=".5" stop-color="#CB4D40"/><stop offset="1" stop-color="#9B302C"/></linearGradient>
    <linearGradient id="button" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FAFBF8"/><stop offset=".48" stop-color="#DEE1DA"/><stop offset=".5" stop-color="#D2D6CE"/><stop offset="1" stop-color="#C0C6BC"/></linearGradient>
    <radialGradient id="halo"><stop stop-color="#FFF1BF" stop-opacity=".65"/><stop offset="1" stop-color="#FFDC81" stop-opacity="0"/></radialGradient>
    <filter id="shadow" x="-40%" y="-20%" width="180%" height="150%"><feDropShadow dx="0" dy="6" stdDeviation="6" flood-color="#000" flood-opacity=".55"/></filter>
    <filter id="iconShadow" x="-40%" y="-40%" width="180%" height="190%"><feDropShadow dx="0" dy="2" stdDeviation="1.4" flood-color="#000" flood-opacity=".5"/></filter>
    <filter id="tinyShadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="1" stdDeviation=".7" flood-color="#000" flood-opacity=".65"/></filter>
    <filter id="blur"><feGaussianBlur stdDeviation="12"/></filter>
    <clipPath id="screenClip">{rect(0,0,W,H,'white')}</clipPath>
    <clipPath id="clientClip">{rect(0,0,W,APP_H,'white',11)}</clipPath>
    </defs>'''

def wallpaper(theme=DEFAULT_THEME):
    t=THEMES[theme]
    s=rect(0,0,W,H,'url(#wall)')
    if theme == 'fx0_gold':
        # Original engraved/foil linework: restrained edge detail with a quiet
        # central card field. All effects are static; no noise or animated sheen.
        s += '<ellipse cx="334" cy="467" rx="275" ry="302" fill="url(#glow)" opacity=".70"/>'
        s += '<ellipse cx="26" cy="180" rx="140" ry="240" fill="url(#glow)" opacity=".13"/>'
        s += f'<path d="M-50 640V0H121Q54 215 83 403T-50 640Z" fill="{t["top"]}" opacity=".24"/>'
        s += f'<g fill="none" stroke="{t["line"]}" stroke-width=".60" opacity=".17">'
        for i in range(17):
            x = 260 + i * 7
            s += f'<path d="M{x} 684 C{x-39} 561 {x-99} 421 {x-44} 269 C{x-8} 169 {x+34} 119 {x+62} 83"/>'
        for i in range(9):
            y = 394 + i * 29
            s += f'<path d="M244 {y} Q304 {y-22} 388 {y-12}"/>'
        for i in range(5):
            x = 4 + i * 8
            s += f'<path d="M{x-44} -20 Q{x+68} 142 {x+10} 321 T{x-31} 647"/>'
        s += '</g>'
        s += f'<g fill="none" stroke="{t["line"]}" stroke-width=".70" opacity=".20"><path d="M341 671C305 549 268 426 309 285"/><path d="M355 664C319 545 283 424 324 283"/></g>'
        s += f'<path d="M342 672C306 550 269 427 310 286" fill="none" stroke="{t["top"]}" stroke-width=".7" opacity=".35"/>'
        return s
    # Wallpaper artwork may recompose vertically; controls and type never scale.
    s+=f'<g transform="scale(1 {H/720})">'
    s+='<ellipse cx="330" cy="472" rx="320" ry="355" fill="url(#glow)"/>'
    s+='<ellipse cx="40" cy="160" rx="190" ry="260" fill="url(#glow)" opacity=".27"/>'
    # Original botanical linework, kept outside the most legible central card area.
    s+=f'<g fill="none" stroke="{t["line"]}" stroke-width=".65" opacity=".16">'
    for j in range(14):
        x=255+j*9
        s+=f'<path d="M {x} 747 C {x-52} 570 {x-140} 480 {x-72} 258 C {x-46} 168 {x+13} 120 {x+45} 95"/>'
    for i in range(9):
        yy=258+i*44
        s+=f'<path d="M {221+i*7} {yy+80} Q {157+i*3} {yy+22} {127+i*10} {yy-35} Q {204+i*5} {yy-9} {221+i*7} {yy+80}"/>'
    s+='</g>'
    s+=f'<g fill="{t["top"]}" opacity=".32"><path d="M-80 680 Q130 520 58 245 Q24 435 -80 680Z"/><path d="M360 720 Q200 600 410 405 Q299 595 360 720Z"/></g>'
    if theme=='heritage':
        s+='<g fill="#D7BF6A" opacity=".18" filter="url(#blur)"><ellipse cx="-12" cy="438" rx="32" ry="191" transform="rotate(-18 -12 438)"/><ellipse cx="349" cy="285" rx="24" ry="220" transform="rotate(22 349 285)"/></g>'
    return s+'</g>'

def glyph(name,x,y,s=20,c='#FFFFFF',sw=1.8):
    # All paths use a 24-unit artboard.
    paths={
      'search':'<circle cx="10" cy="10" r="6.6"/><path d="M15 15 22 22"/>',
      'chevron':'<path d="m9 5 7 7-7 7"/>',
      'back':'<path d="m15 5-7 7 7 7"/>',
      'plus':'<path d="M12 4v16M4 12h16"/>',
      'check':'<path d="m4 12 5 5L21 5"/>',
      'close':'<path d="m5 5 14 14M19 5 5 19"/>',
      'up':'<path d="m4 12 8-9 8 9h-5v8H9v-8Z"/>',
      'wifi':'<path d="M2 8q10-9 20 0M6 12q6-6 12 0M10 16q2-2 4 0"/><circle cx="12" cy="20" r=".8" fill="currentColor"/>',
      'bluetooth':'<path d="m7 6 12 12-7 5V1l7 5L7 18"/>',
      'plane':'<path d="m2 12 9-2V3q1-4 2 0v7l9 2v3l-9-1v6l3 1v2l-4-1-4 1v-2l3-1v-6l-9 1Z"/>',
      'speaker':'<path d="M3 9h5l6-5v16l-6-5H3ZM17 8q5 4 0 8M20 5q8 7 0 14"/>',
      'mic':'<rect x="9" y="2" width="6" height="13" rx="3"/><path d="M5 10v2q0 8 7 8t7-8v-2M12 20v3M7 23h10"/>',
      'lock':'<rect x="5" y="10" width="14" height="12" rx="2"/><path d="M8 10V6q0-8 8 0v4"/><path d="M12 15v3"/>',
      'refresh':'<path d="M20 8A9 9 0 1 0 20 17M20 3v6h-6"/>',
      'trash':'<path d="M5 7h14M8 7V4h8v3M6 7l1 15h10l1-15M10 11v7M14 11v7"/>',
      'pencil':'<path d="m4 16 12-13 5 5L8 21l-6 1ZM14 5l5 5"/>',
      'power':'<path d="M12 1v11M6 5a10 10 0 1 0 12 0"/>',
      'sun':'<circle cx="12" cy="12" r="4"/><path d="M12 0v4M12 20v4M0 12h4M20 12h4M3 3l3 3M18 18l3 3M3 21l3-3M18 6l3-3"/>',
    }
    return f'<g transform="translate({x} {y}) scale({s/24})" fill="none" stroke="{c}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round">{paths.get(name,paths["plus"])}</g>'

def icon(kind,x,y,size=56):
    s=f'<g transform="translate({x} {y}) scale({size/64})" filter="url(#iconShadow)">'
    if kind=='phone':
        s+='<path d="M13 6Q5 9 7 24q7 27 33 34 15 3 18-10l-12-9q-4-2-9 6-13-5-18-18 9-6 6-10Z" fill="url(#green)" stroke="#294C29" stroke-width="1.7"/><path d="M13 10q-4 11 1 19" fill="none" stroke="#F2FFCA" stroke-width="2.5" opacity=".8"/><path d="M37 8q17 2 19 19M38 16q9 1 10 10" fill="none" stroke="#D7EDAA" stroke-width="3.5" stroke-linecap="round"/>'
    elif kind=='contacts':
        s+='<g transform="rotate(-8 32 32)"><rect x="8" y="8" width="46" height="47" rx="3" fill="#E8DFCA" stroke="#736B56"/><path d="M8 8h46v8H8Z" fill="url(#gold)"/><path d="M16 4v11M26 4v11M36 4v11M46 4v11" stroke="#BEB8A6" stroke-width="4"/>'+circle(24,29,7,'#BE895F')+'<path d="M13 49v-5q0-10 11-10t11 10v5" fill="#6F9A9B"/><path d="M39 27h9M39 35h9M39 43h9" stroke="#797C71" stroke-width="2"/></g>'
    elif kind=='mail':
        s+='<g transform="rotate(-10 32 33)"><rect x="5" y="15" width="54" height="36" rx="2" fill="url(#surface)" stroke="#6C7677" stroke-width="1.3"/><path d="M6 49 26 31M58 49 38 31" fill="none" stroke="#A6ADA6"/><path d="m5 15 27 22 27-22" fill="#FAFBF6" stroke="#8A9699"/><path d="M8 15h48" stroke="#C4D9E6" stroke-width="3"/><path d="M8 51h48" stroke="#7598AD" stroke-width="2"/></g>'
    elif kind=='calendar':
        s+='<g transform="rotate(3 32 32)"><rect x="8" y="8" width="48" height="50" rx="3" fill="url(#surface)" stroke="#727A77"/><path d="M8 9h48v14H8Z" fill="url(#blue)"/><path d="M20 4v12M44 4v12" stroke="#D5DDD9" stroke-width="4" stroke-linecap="round"/>'+text(32,48,'28',30,'#20282B','bold','middle')+'</g>'
    elif kind=='launcher':
        s+=circle(32,33,27,'url(#dark)','#B5C0B9',1.2)+circle(32,33,23,'none','#131B20',1.2)+glyph('up',17,15,31,'#E4EAE4',1.8)
    elif kind=='web':
        s+=circle(32,32,26,'url(#blue)','#214C65',1.3)+'<path d="m23 9-8 10 5 5-2 10 13 3 4 18 7-8-3-10 11-10-5-7-9 2-4-8Z" fill="#94B95D" stroke="#698A44" stroke-width="1"/><path d="M11 16q10-13 24-8" stroke="#D7F5F9" stroke-width="3" fill="none" opacity=".8"/>'
    elif kind=='messages':
        s+='<path d="M5 10q28-9 54 0v31q-7 5-19 5L27 58l1-12Q12 47 5 41Z" fill="url(#green)" stroke="#52733B"/><path d="M12 14q20-6 40 0" stroke="#EEF8D3" stroke-width="2" fill="none"/>'+text(32,35,'···',28,'#F5F9E4','bold','middle')
    elif kind=='camera':
        s+='<path d="M6 18h12l5-7h19l5 7h11v35H6Z" fill="url(#dark)" stroke="#BBC1BE" stroke-width="1.5"/><rect x="7" y="20" width="50" height="7" fill="#B8BDB6"/>'+circle(33,37,15,'#182126','#87999C',3)+circle(33,37,10,'url(#blue)','#111B21',2)+circle(29,33,3,'#D9F2F6')+rect(48,30,6,4,'#D6E5AF',1)
    elif kind=='photos':
        s+='<g transform="rotate(-9 32 32)"><rect x="8" y="11" width="46" height="42" fill="#F4F2E9" stroke="#A6ACA9"/><rect x="12" y="15" width="38" height="30" fill="url(#blue)"/><path d="m12 42 13-19 10 11 7-6 8 17H12Z" fill="#729151"/><circle cx="41" cy="22" r="5" fill="#EDCD78"/></g>'
    elif kind=='music':
        s+='<path d="M21 14 50 6v37" fill="none" stroke="#B9C8C8" stroke-width="8" stroke-linejoin="round"/><path d="M21 14v38" stroke="#CAD5D0" stroke-width="6"/>'+circle(15,51,10,'url(#blue)','#4D6A7B',1)+circle(44,44,10,'url(#blue)','#4D6A7B',1)+'<path d="m23 14 25-7" stroke="#E6EDE7" stroke-width="2"/>'
    elif kind=='clock':
        s+=circle(32,32,27,'url(#dark)','#B4BCB8',2)+circle(32,32,22,'#E9ECE4','#182429',1)+'<path d="M32 14v19l12 7" fill="none" stroke="#263337" stroke-width="3" stroke-linecap="round"/>'+circle(32,32,3,'#B59C68')
        for a in range(12):
            aa=a*math.pi/6;s+=line(32+18*math.sin(aa),32-18*math.cos(aa),32+20*math.sin(aa),32-20*math.cos(aa),'#56676A',1.5)
    elif kind=='notes':
        s+='<g transform="rotate(5 32 32)"><path d="M9 7h46v50H9Z" fill="#E8D595" stroke="#A79666"/><path d="M9 8h46v9H9Z" fill="#BE9E5A"/><path d="M19 5v15M29 5v15M39 5v15M49 5v15" stroke="#5D6664" stroke-width="3"/><path d="M16 28h31M16 36h31M16 44h20" stroke="#B9AB76" stroke-width="1.2"/></g>'
    elif kind=='settings':
        s+='<g fill="#AEBAB9" stroke="#4D6267" stroke-width="1.3">'
        for a in range(8):s+=f'<rect x="27" y="3" width="10" height="15" rx="2" transform="rotate({a*45} 32 32)"/>'
        s+=circle(32,32,21,'url(#chrome)','#51686D',1.7)+circle(32,32,10,'url(#blue)','#52686B',2)+'</g>'
    elif kind=='files':
        s+='<path d="M5 15h23l6 6h25v34H5Z" fill="#A9844E" stroke="#665740"/><path d="M5 24h54l-5 31H1Z" fill="url(#gold)" stroke="#8A724C"/><path d="M8 28h44" stroke="#F4DA95" stroke-width="2"/>'
    elif kind=='terminal':
        s+=rect(5,9,54,45,'url(#dark)',4,'#ADBDB8',1.5)+rect(7,11,50,6,'#647671',2)+text(13,43,'>_',25,'#BDDEAA','bold',family='DejaVu Sans Mono')
    elif kind=='store':
        s+='<path d="M13 17h38l5 40H8Z" fill="#D9CBA9" stroke="#8A8069"/><path d="M23 23V12q9-15 18 0v11" fill="none" stroke="#8D9B99" stroke-width="4"/>'+text(32,48,'+',29,'#F8FAEF','bold','middle')
    else:
        s+=circle(32,32,26,'url(#blue)')+glyph('plus',20,20,24)
    return s+'</g>'

def status(title='Tarot', menu=False, light=True, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, 0, 360, 32, p['status.background'])
    if menu:
        s += rect(0, 1, min(128, 29 + len(title) * 7.3), 30, 'url(#uiDark)', 4)
    s += text(9, 22, title, 15, p['status.title'], 'bold')
    if menu:
        s += f'''<path d="m{15 + len(title) * 7.4} 14 7 0-3.5 4Z" fill="{p['status.menu_arrow']}"/>'''
    s += text(180, 22, '9:41', 16, p['status.clock'], 'bold', 'middle')
    s += text(278, 21, '4G', 10.5, p['status.network_label'], 'bold', 'middle')
    for i in range(4):
        s += rect(290 + i * 4.6, 22 - (i + 1) * 3.6, 3.2, (i + 1) * 3.6, p['status.network_glyph'], 0.5)
    s += glyph('wifi', 311, 8, 17, p['status.network_glyph'], 2)
    s += rect(336, 7, 14, 20, 'none', 2, p['status.battery_border'], 1) + rect(339, 10, 8, 14, p['status.battery_charge'], 1) + rect(340, 4, 6, 3, p['status.battery_cap'], 0.6)
    return s

def gesture(active=False, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, GESTURE_Y, W, GESTURE_H, p['gesture.background'])
    s += line(0, GESTURE_Y, W, GESTURE_Y, p['gesture.edge'], 0.55)
    if active:
        s += f'<ellipse cx="180" cy="{GESTURE_Y + 18}" rx="53" ry="13" fill="url(#uiHalo)" opacity=".7"/>'
    s += rect(161, GESTURE_Y + 17, 38, 2, p['gesture.active_indicator'] if active else p['gesture.idle_indicator'], 1)
    return s

def frame_corners(top=STATUS_H, bottom=GESTURE_Y, r=11, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    return f'''<g fill="{p['frame_corners.fill']}"><path d="M0 {top}H{r}Q0 {top} 0 {top + r}Z"/><path d="M360 {top}H{360 - r}Q360 {top} 360 {top + r}Z"/><path d="M0 {bottom}H{r}Q0 {bottom} 0 {bottom - r}Z"/><path d="M360 {bottom}H{360 - r}Q360 {bottom} 360 {bottom - r}Z"/></g>'''

def just_type(theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    return rect(9, 40, 342, 40, p['just_type.field'], 20, p['just_type.border'], 0.6, extra='fill-opacity=".25" stroke-opacity=".38"') + text(25, 67, 'Just type...', 20, p['just_type.placeholder'], extra='font-style="italic" opacity=".78"') + glyph('search', 318, 49, 21, p['just_type.glyph'], 2)

def dock(wave=False, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = f'<g transform="translate(0 {BOTTOM_SHIFT})">'
    if not wave:
        s += rect(3, 611, 354, 69, 'url(#uiTray)', 18, p['dock.border'], 0.8) + line(20, 612, 340, 612, p['dock.highlight'], 0.7)
        for i, k in enumerate(['phone', 'contacts', 'mail', 'calendar', 'launcher']):
            s += icon(k, 10 + i * 71, 619, 56)
    else:
        s += f'''<path d="M-5 651 C61 654 79 580 142 574 C205 565 241 610 365 654 L365 684H-5Z" fill="url(#uiTray)" stroke="{p['dock.wave_border']}" stroke-opacity=".7" stroke-width=".8"/>'''
        ys = [626, 593, 562, 593, 626]
        for i, k in enumerate(['phone', 'contacts', 'mail', 'calendar', 'launcher']):
            if i == 2:
                s += '<ellipse cx="180" cy="594" rx="51" ry="48" fill="url(#uiHalo)"/>'
            s += icon(k, 10 + i * 71, ys[i], 56)
        s += text(180, 551, 'Email', 15, p['dock.wave_label'], 'bold', 'middle', extra='filter="url(#tinyShadow)"')
    return s + '</g>'

def app_header(title, kind=None, right=None, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, 0, 360, 56, 'url(#uiChrome)') + line(0, 55.5, 360, 55.5, p['app_header.border'], 1) + line(0, 1, 360, 1, p['app_header.highlight'], 1)
    if kind:
        s += icon(kind, 9, 9, 35)
        x = 51
    else:
        x = 16
    s += text(x, 37, title, 24, p['app_header.title'])
    if right:
        s += text(341, 35, right, 15, p['app_header.action'], 'bold', 'end')
    return s

def command_bar(y=APP_H - 56, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, y, 360, 56, 'url(#uiChrome)') + line(0, y, 360, y, p['command_bar.border'], 1)
    s += rect(118, y + 8, 124, 39, 'url(#uiDark)', 18, p['command_bar.control_border'], 0.7)
    s += glyph('refresh', 134, y + 16, 21, p['command_bar.glyph']) + line(178, y + 10, 178, y + 44, p['command_bar.separator'], 0.8) + glyph('pencil', 199, y + 16, 21, p['command_bar.glyph'])
    return s

def email_content(height=APP_H, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, 0, 360, height, p['email_content.surface']) + app_header('Inbox', 'mail', 'Edit', theme=theme)
    s += rect(9, 64, 342, 35, p['email_content.search_surface'], 17, p['email_content.search_border'], 0.8) + glyph('search', 20, 72, 18, p['email_content.search_glyph']) + text(47, 88, 'Search mail', 17, p['email_content.search_placeholder'])
    s += rect(0, 107, 360, 25, p['email_content.section_surface']) + text(14, 125, 'TODAY', 12, p['email_content.section_label'], 'bold')
    rows = [('Mara Ellis', 'Saturday by the water', 'The weather looks good. Shall we meet...', '9:28', True), ('Jun Park', 'Photos from the coast', 'I finally sorted the pictures from our trip.', '8:52', True), ('The Bookshop', 'Your order is ready', 'Your books are waiting at the front desk.', '8:10', False), ('Daniel Moore', 'Dinner on Friday', 'I booked a table for six. See you there.', '7:46', False), ('Mara Ellis', 'A little list', 'Tea, bread, lemons, and something sweet.', 'Yesterday', False), ('Garden Club', 'A new season', 'Our next gathering is on Sunday.', 'Yesterday', False)]
    y = 132
    for name, subject, body, tm, unread in rows:
        if y + 73 > height - 56:
            break
        s += rect(0, y, 360, 73, 'url(#uiRow)') + line(0, y + 72, 360, y + 72, p['email_content.divider'], 0.8)
        if unread:
            s += circle(12, y + 20, 3.2, p['email_content.unread_dot'])
        s += text(24, y + 22, name, 18, p['email_content.primary_text'], 'bold' if unread else 'normal') + text(345, y + 21, tm, 12, p['email_content.metadata'], anchor='end')
        s += text(24, y + 44, subject, 16, p['email_content.subject'], 'bold' if unread else 'normal') + text(24, y + 63, body, 13.4, p['email_content.preview'])
        y += 73
    s += command_bar(height - 56, theme=theme)
    return s

def calendar_content(height=APP_H, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, 0, 360, height, p['calendar_content.surface']) + app_header('Wednesday 28', 'calendar', theme=theme)
    s += rect(0, 56, 360, 35, 'url(#uiButton)') + text(180, 80, 'October 2026', 17, p['calendar_content.month'], 'bold', 'middle') + glyph('back', 15, 63, 18, p['calendar_content.navigation']) + glyph('chevron', 326, 63, 18, p['calendar_content.navigation'])
    for i, h in enumerate(range(8, 18)):
        y = 91 + i * 45
        if y > height - 56:
            break
        s += text(12, y + 16, f'{h:02}:00', 12, p['calendar_content.hours']) + line(58, y + 5, 360, y + 5, p['calendar_content.grid'], 0.6)
    s += rect(64, 163.5, 285, 45, p['calendar_content.event_a_surface'], 5, p['calendar_content.event_a_border'], 0.6) + rect(64, 163.5, 4, 45, p['calendar_content.event_a_rail'], 2) + text(77, 183, 'Coffee with Jun', 17, p['calendar_content.event_a_title'], 'bold') + text(77, 201, '9:30 - 10:30', 13, p['calendar_content.event_a_metadata'])
    s += rect(64, 298.5, 285, 67.5, p['calendar_content.event_b_surface'], 5, p['calendar_content.event_b_border'], 0.6) + rect(64, 298.5, 4, 67.5, p['calendar_content.event_b_rail'], 2) + text(77, 322, 'Walk by the water', 17, p['calendar_content.event_b_title'], 'bold') + text(77, 346, '12:30 - 14:00', 13, p['calendar_content.event_b_metadata'])
    yy = height - 56
    s += rect(0, yy, 360, 56, 'url(#uiChrome)') + line(0, yy, 360, yy, p['calendar_content.toolbar_border'], 1) + rect(75, yy + 8, 209, 39, 'url(#uiDark)', 18, p['calendar_content.control_border'], 0.7)
    for j, lab in enumerate(['Day', 'Week', 'Month']):
        xx = 110 + j * 70
        s += text(xx, yy + 33, lab, 15, p['calendar_content.active_tab'] if j == 0 else p['calendar_content.inactive_tab'], 'bold' if j == 0 else 'normal', 'middle')
        if j < 2:
            s += line(xx + 35, yy + 11, xx + 35, yy + 44, p['calendar_content.separator'], 0.8)
    return s

def message_content(height=APP_H, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, 0, 360, height, p['message_content.surface']) + app_header('Mara Ellis', 'messages', theme=theme) + text(180, 82, 'Today', 13, p['message_content.date'], anchor='middle')
    for x, y, w, h, txt, fill in [(14, 99, 278, 72, 'Shall we take the long way home?', p['message_content.received_surface']), (69, 191, 276, 52, 'Yes. Meet you by the water.', p['message_content.sent_surface']), (14, 268, 280, 72, 'I will bring the map and some tea.', p['message_content.received_surface'])]:
        s += rect(x, y, w, h, fill, 10, p['message_content.bubble_border'], 0.7)
        if h > 60:
            words = txt.split()
            mid = 5 if len(words) > 8 else 4
            s += text(x + 13, y + 27, ' '.join(words[:mid]), 17, p['message_content.message_text']) + text(x + 13, y + 51, ' '.join(words[mid:]), 17, p['message_content.message_text'])
        else:
            s += text(x + 13, y + 32, txt, 17, p['message_content.message_text'])
    s += rect(9, height - 60, 342, 48, 'url(#uiRow)', 8, p['message_content.compose_border'], 0.8) + text(24, height - 29, 'Write a message...', 17, p['message_content.compose_placeholder']) + glyph('plus', 314, height - 47, 22, p['message_content.compose_glyph'])
    return s

def web_content(height=APP_H, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, 0, 360, height, p['web_content.surface']) + app_header('The quiet coast', 'web', theme=theme)
    s += rect(10, 66, 340, 33, p['web_content.address_surface'], 7, p['web_content.address_border'], 0.7) + text(24, 88, 'example.org / coast', 15, p['web_content.address_text'])
    s += rect(0, 110, 360, 214, '#859F99') + '<path d="M0 230Q120 160 240 250T400 240V324H0Z" fill="#607E7B"/><path d="M0 292Q150 237 360 286V324H0Z" fill="#CDD0B6"/>'
    s += text(24, 366, 'A day beside', 30, p['web_content.heading'], family='Instrument Serif') + text(24, 399, 'the water', 30, p['web_content.heading'], family='Instrument Serif')
    for i, tx in enumerate(['Take the path that follows the coast.', 'There is time to stop, look around,', 'and take the long way home.']):
        s += text(24, 438 + i * 25, tx, 17, p['web_content.body_text'])
    s += command_bar(height - 56, theme=theme)
    return s

def group_box(x, y, w, label, rows, accent=None, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    accent = p['settings.value_accent'] if accent is None else accent
    hh = 28 + len(rows) * 51
    s = rect(x, y, w, hh, 'url(#uiRow)', 10, p['group_box.border'], 1) + rect(x + 1, y + 1, w - 2, 28, p['group_box.heading_surface'], 9) + rect(x + 1, y + 18, w - 2, 10, p['group_box.heading_surface']) + text(x + 11, y + 20, label, 13, p['group_box.heading_label'], 'bold')
    for i, (lab, value, typ) in enumerate(rows):
        yy = y + 28 + i * 51
        if i:
            s += line(x + 1, yy, x + w - 1, yy, p['group_box.divider'], 0.8)
        s += text(x + 13, yy + 33, lab, 19, p['group_box.row_label'])
        if typ == 'toggle':
            on = value == 'On'
            s += rect(x + w - 80, yy + 14, 65, 28, 'url(#uiAccent)' if on else p['group_box.switch_off_surface'], 14, p['group_box.switch_border'], 0.8) + circle(x + w - (28 if on else 66), yy + 28, 12, 'url(#uiButton)', p['group_box.switch_knob_border'], 0.7) + text(x + w - (57 if on else 37), yy + 32, 'ON' if on else 'OFF', 10, p['switch.on_label'] if on else p['switch.off_label'], 'bold', 'middle')
        elif typ == 'chevron':
            s += text(x + w - 34, yy + 32, value, 15, p['group_box.row_value'], anchor='end') + glyph('chevron', x + w - 25, yy + 17, 17, p['group_box.chevron'])
        else:
            s += text(x + w - 15, yy + 32, value, 16, accent, anchor='end')
    return s

def settings_content(height=APP_H, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, 0, 360, height, p['settings_content.surface']) + app_header('Screen & Lock', 'settings', theme=theme)
    s += rect(10, 67, 340, 130, 'url(#uiRow)', 10, p['settings_content.border'], 1) + rect(11, 68, 338, 28, p['settings_content.heading_surface'], 9) + rect(11, 85, 338, 10, p['settings_content.heading_surface']) + text(22, 87, 'SCREEN', 13, p['settings_content.heading_label'], 'bold') + text(24, 125, 'Brightness', 20, p['settings_content.row_label'])
    s += rect(27, 149, 306, 5, p['settings_content.slider_track'], 2) + rect(27, 149, 178, 5, p['settings_content.slider_fill'], 2) + circle(205, 151, 14, 'url(#uiChrome)', p['settings_content.slider_knob_border'], 1) + circle(205, 151, 9, p['settings_content.slider_knob_center'])
    s += group_box(10, 208, 340, 'DISPLAY', [('Turn off after', '1 minute', 'chevron'), ('Wallpaper', THEMES[theme]['name'], 'chevron')], theme=theme)
    s += group_box(10, 349, 340, 'GESTURES', [('Switch applications', 'Off', 'toggle'), ('Gesture hints', 'On', 'toggle')], theme=theme)
    s += group_box(10, 490, 340, 'SECURITY', [('Screen lock', 'PIN', 'chevron')], theme=theme)
    return s

CONTENTS={'email':email_content,'calendar':calendar_content,'messages':message_content,'web':web_content,'settings':settings_content}

def card(kind, x, y, scale=0.66, angle=0, opacity=1, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    cont = CONTENTS[kind](APP_H, theme=theme)
    return f'''<g transform="translate({x} {y}) rotate({angle} {W / 2 * scale} {APP_H / 2 * scale})" opacity="{opacity}"><g transform="scale({scale})">{rect(0, 0, W, APP_H, p['card.backing'], 11, extra='filter="url(#shadow)"')}<g clip-path="url(#clientClip)">{cont}</g>{rect(0.5, 0.5, W - 1, APP_H - 1, 'none', 11, p['card.border'], 1)}</g></g>'''

def overview(theme=DEFAULT_THEME, stack=False, empty=False):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + status(theme=theme) + just_type(theme=theme)
    if not empty:
        if stack:
            s += card('calendar', -209, 137, 0.61, -2, 0.9, theme=theme)
            s += card('email', 43, 124, 0.64, -5, 0.95, theme=theme) + card('messages', 62, 118, 0.64, 0, 0.98, theme=theme) + card('web', 81, 124, 0.64, 5, 1, theme=theme)
        else:
            s += card('calendar', -204, 133, 0.61, 0, 0.9, theme=theme) + card('messages', 345, 133, 0.61, 0, 0.9, theme=theme) + card('email', 61.2, CARD_Y, 0.66, theme=theme)
    s += dock(theme=theme) + gesture(theme=theme)
    return s

def launcher(theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + rect(0, STATUS_H, W, APP_H, p['launcher.scrim'], extra='fill-opacity=".50"') + status('Launcher', True, theme=theme)
    s += text(180, 70, 'Applications', 21, p['launcher.page_label'], anchor='middle', extra='filter="url(#tinyShadow)"')
    s += glyph('back', 15, 50, 18, p['launcher.previous_page']) + glyph('chevron', 327, 50, 18, p['launcher.next_page'])
    apps = [('phone', 'Phone'), ('web', 'Web'), ('store', 'Software'), ('mail', 'Email'), ('calendar', 'Calendar'), ('messages', 'Messaging'), ('camera', 'Camera'), ('photos', 'Photos'), ('music', 'Music'), ('clock', 'Clock'), ('notes', 'Memos'), ('settings', 'Settings'), ('contacts', 'Contacts'), ('files', 'Files'), ('terminal', 'Terminal')]
    for i, (kind, title) in enumerate(apps):
        col = i % 3
        row = i // 3
        x = 29 + col * 122
        y = 95 + row * 96
        s += icon(kind, x, y, 58) + text(x + 29, y + 77, title, 14, p['launcher.app_label'], anchor='middle', extra='filter="url(#tinyShadow)"')
    s += f'''<path d="m174 582 12 0-6 7Z" fill="{p['launcher.scroll_cue']}"/>'''
    s += frame_corners(theme=theme) + gesture(theme=theme)
    return s

def full_app(kind='email', theme=DEFAULT_THEME, banner=False, dashboard=False, wave=False, device_menu=False, app_menu=False):
    p = UI_ROLES[theme]
    title = {'email': 'Email', 'calendar': 'Calendar', 'messages': 'Messaging', 'web': 'Web', 'settings': 'Preferences'}[kind]
    s = wallpaper(theme) + status(title, True, theme=theme)
    h = APP_H - BANNER_H if banner else APP_H - DASHBOARD_H if dashboard else APP_H
    s += f"""<defs><clipPath id="appContentClip">{rect(0, 0, W, h, 'white')}</clipPath></defs>"""
    s += f'<g transform="translate(0 {STATUS_H})"><g clip-path="url(#appContentClip)">{CONTENTS[kind](h, theme=theme)}</g></g>'
    s += frame_corners(STATUS_H, STATUS_H + h, theme=theme)
    if banner:
        yy = GESTURE_Y - BANNER_H
        s += rect(0, yy, W, BANNER_H, p['full_app.banner_surface']) + icon('messages', 5, yy + 2, 27) + text(40, yy + 22, 'Mara: I will bring the map.', 15, p['full_app.banner_text'])
    if dashboard:
        dashboard_y = GESTURE_Y - DASHBOARD_H
        s += rect(0, dashboard_y, W, DASHBOARD_H, p['full_app.dashboard_surface'])
        items = [('messages', 'Mara Ellis', 'I will bring the map.', 2), ('calendar', 'Walk by the water', 'Today 12:30', 0), ('music', 'The long way home', 'Juniper / Evening light', 0)]
        for i, (ic, a, b, n) in enumerate(items):
            yy = dashboard_y + 9 + i * 56
            s += rect(0, yy, W, 53, 'url(#uiDark)', 12)
            s += icon(ic, 9, yy + 7, 38) + text(59, yy + 23, a, 17, p['full_app.dashboard_title'], 'bold') + text(59, yy + 43, b, 14, p['full_app.dashboard_secondary'])
            if n:
                s += circle(45, yy + 9, 9, p['full_app.badge_surface'], p['full_app.badge_border'], 0.6) + text(45, yy + 13, str(n), 10, 'white', 'bold', 'middle')
    if wave:
        s += rect(0, STATUS_H, W, APP_H, p['full_app.wave_scrim'], extra='fill-opacity=".23"') + dock(True, theme=theme)
    if device_menu:
        s += rect(0, STATUS_H, W, APP_H, p['full_app.overlay'], extra='fill-opacity=".30"')
        s += rect(107, 32, 249, 337, 'url(#uiMenu)', 10, p['full_app.device_menu_border'], 0.65, extra='filter="url(#shadow)"') + rect(107, 32, 249, 15, 'url(#uiMenu)')
        s += text(125, 63, 'Wednesday, October 28', 16, p['full_app.menu_heading'], 'bold') + text(125, 89, 'Battery 82%', 15, p['full_app.menu_secondary']) + line(116, 105, 347, 105, p['full_app.menu_separator'], 1)
        for i, (g, title, val) in enumerate([('wifi', 'Wi-Fi', 'Home'), ('bluetooth', 'Bluetooth', 'On'), ('plane', 'Airplane mode', 'Off')]):
            yy = 119 + i * 57
            s += glyph(g, 124, yy + 3, 22, p['full_app.menu_glyph']) + text(158, yy + 21, title, 17, p['full_app.menu_label']) + text(334, yy + 41, val, 12, p['full_app.menu_value'], anchor='end') + line(116, yy + 49, 347, yy + 49, p['full_app.menu_row_separator'], 1)
        s += glyph('sun', 124, 302, 21, p['full_app.slider_glyph']) + rect(158, 311, 167, 5, p['full_app.slider_track'], 2) + rect(158, 311, 105, 5, p['full_app.slider_fill'], 2) + circle(263, 313, 11, 'url(#uiChrome)', p['full_app.slider_border'], 0.6)
        s += text(233, 353, 'Preferences', 15, p['full_app.preferences_label'], anchor='middle')
    if app_menu:
        s += rect(0, STATUS_H, W, APP_H, p['full_app.overlay'], extra='fill-opacity=".25"') + rect(4, 32, 235, 259, 'url(#uiMenu)', 10, p['full_app.app_menu_border'], 0.65, extra='filter="url(#shadow)"') + rect(4, 32, 235, 10, 'url(#uiMenu)')
        for i, tt in enumerate(['Edit', 'New message', 'Preferences & Accounts', 'Help']):
            yy = 41 + i * 58
            s += text(21, yy + 32, tt, 18, p['full_app.app_menu_label'])
            if i == 0:
                s += glyph('chevron', 209, yy + 19, 16, p['full_app.submenu_glyph'])
            if i < 3:
                s += line(11, yy + 51, 232, yy + 51, p['full_app.app_menu_separator'], 1)
    s += gesture(wave, theme=theme)
    return s

def keyboard(y=KEYBOARD_Y, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = rect(0, y, W, KEYBOARD_H, p['keyboard.surface']) + line(0, y, W, y, p['keyboard.edge'], 1)
    for row, chars in enumerate(['qwertyuiop', 'asdfghjkl', 'zxcvbnm']):
        xx = 3 if row == 0 else 21 if row == 1 else 57
        for i, ch in enumerate(chars):
            x = xx + i * 35.5
            yy = y + 10 + row * 48
            s += rect(x, yy, 32, 42, 'url(#uiButton)', 5, p['keyboard.key_border'], 0.75) + text(x + 16, yy + 29, ch, 22, p['keyboard.key_label'], anchor='middle')
    s += rect(3, y + 106, 46, 42, 'url(#uiDark)', 5) + text(26, y + 135, '⇧', 22, p['keyboard.shift_label'], anchor='middle')
    s += rect(309, y + 106, 48, 42, 'url(#uiDark)', 5) + glyph('back', 321, y + 117, 22, p['keyboard.back_glyph'])
    for x, w, tx in [(3, 54, '?123'), (61, 39, ','), (104, 153, 'space'), (261, 36, '.'), (301, 56, 'Go')]:
        s += rect(x, y + 159, w, 47, 'url(#uiDark)' if tx == 'Go' else 'url(#uiButton)', 5, p['keyboard.bottom_key_border'], 0.7) + text(x + w / 2, y + 189, tx, 15, p['keyboard.go_label'] if tx == 'Go' else p['keyboard.bottom_key_label'], 'bold' if tx == 'Go' else 'normal', 'middle')
    return s

def search_screen(theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + status('Just Type', True, theme=theme) + rect(0, STATUS_H, W, KEYBOARD_Y - STATUS_H, p['search_screen.surface'])
    s += rect(10, 43, 340, 41, p['search_screen.field'], 20, p['search_screen.field_border'], 0.9) + glyph('search', 24, 54, 19, p['search_screen.glyph']) + text(53, 71, 'Mara', 20, p['search_screen.query']) + line(96, 51, 96, 75, p['search_screen.caret'], 1.2) + glyph('close', 320, 56, 15, p['search_screen.clear_glyph'])
    s += f"""<defs><clipPath id="searchResultsClip">{rect(0, 92, W, KEYBOARD_Y - 92, 'white')}</clipPath></defs><g clip-path="url(#searchResultsClip)">"""
    for y, tx in [(97, 'CONTACTS'), (194, 'CONTENT'), (287, 'ACTIONS')]:
        s += rect(0, y, W, 25, p['search_screen.section_surface']) + text(14, y + 18, tx, 12, p['search_screen.section_label'], 'bold')
    s += icon('contacts', 13, 132, 48) + text(74, 151, 'Mara Ellis', 20, p['search_screen.result_title'], 'bold') + text(74, 174, 'Mobile  ·  Send a message', 14, p['search_screen.result_subtitle'])
    s += icon('photos', 15, 230, 43) + text(73, 249, 'Photos', 20, p['search_screen.result_title']) + text(73, 270, '3 photos with Mara', 13, p['search_screen.result_metadata'])
    s += text(20, 342, 'New message', 19, p['search_screen.action']) + text(20, 401, 'Search the web for “Mara”', 18, p['search_screen.action'])
    s += f'''</g><path d="m174 370 12 0-6 7Z" fill="{p['search_screen.scroll_cue']}"/>'''
    s += keyboard(theme=theme) + gesture(theme=theme)
    return s

def notification_rail(theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    rail_y = GESTURE_Y - BANNER_H
    s = wallpaper(theme) + f'<g transform="translate(0 {STATUS_H})">{email_content(APP_H - BANNER_H, theme=theme)}</g>' + frame_corners(STATUS_H, rail_y, theme=theme)
    s += rect(0, rail_y, W, BANNER_H, p['notification_rail.surface'])
    for i, k in enumerate(['messages', 'calendar', 'mail']):
        s += icon(k, 255 + i * 32, rail_y + 3, 25)
    s += gesture(theme=theme)
    return s

def launch_pending(theme=DEFAULT_THEME, failed=False):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + just_type(theme=theme)
    card_h = APP_H * 0.66
    s += rect(61.2, CARD_Y, W * 0.66, card_h, 'url(#uiDark)', 8, p['launch_pending.border'], 0.7, extra='filter="url(#shadow)"')
    s += f'<g transform="translate(0 {CARD_Y + card_h / 2 - (139 + 430 / 2)})">'
    s += icon('mail', 148, 276, 64)
    if failed:
        s += text(180, 373, 'Email did not open', 18, p['launch_pending.title'], 'bold', 'middle') + text(180, 401, 'Try opening it again.', 15, p['launch_pending.secondary'], anchor='middle')
        s += rect(111, 438, 138, 43, 'url(#uiButton)', 8, p['launch_pending.retry_border'], 0.8) + text(180, 466, 'Try again', 17, p['launch_pending.retry_label'], 'bold', 'middle')
    else:
        s += text(180, 373, 'Email', 19, p['launch_pending.title'], 'bold', 'middle')
        for i in range(3):
            s += circle(166 + i * 14, 402, 3, p['launch_pending.active_dot'] if i == 0 else p['launch_pending.idle_dot'])
    return s + '</g>' + dock(theme=theme) + gesture(theme=theme)

def lock_screen(theme=DEFAULT_THEME, pin=False):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + rect(0, 0, W, H, p['lock_screen.scrim'], extra=('fill-opacity=".50"' if theme == 'fx0_gold' else 'fill-opacity=".18"'))
    s += status('Tarot', theme=theme)
    if not pin:
        s += text(180, 191, '9:41', 70, p['lock_screen.clock'], 'normal', 'middle', extra='filter="url(#tinyShadow)"') + text(180, 225, 'Wednesday, October 28', 18, p['lock_screen.date'], anchor='middle')
        s += rect(16, 280, 328, 56, p['lock_screen.notification_surface'], 12, p['lock_screen.notification_border'], 0.6, extra='fill-opacity=".74"') + icon('messages', 25, 290, 35) + text(76, 303, 'Messaging', 16, p['lock_screen.notification_title'], 'bold') + text(76, 323, '2 new messages', 14, p['lock_screen.notification_secondary'])
        s += f'''<circle cx="180" cy="{GESTURE_Y - 106}" r="61" fill="{p['lock_screen.target_surface']}" fill-opacity=".34" stroke="{p['lock_screen.target_border']}" stroke-opacity=".16"/>'''
        s += circle(180, GESTURE_Y - 90, 32, 'url(#uiDark)', p['lock_screen.handle_border'], 1.2) + glyph('lock', 167, GESTURE_Y - 105, 26, p['lock_screen.lock_glyph'], 1.8)
        s += text(180, GESTURE_Y - 33, 'Drag up to unlock', 16, p['lock_screen.instruction'], anchor='middle')
    else:
        s += text(180, 140, 'Enter PIN', 28, p['lock_screen.pin_title'], anchor='middle')
        for x in [144, 168, 192, 216]:
            s += circle(x, 177, 4, p['lock_screen.pin_indicator'] if x < 192 else 'none', p['lock_screen.pin_indicator'], 1)
        for i in range(9):
            x = 47 + i % 3 * 104
            y = 225 + i // 3 * 76
            s += rect(x, y, 58, 59, 'url(#uiTray)', 10, p['lock_screen.key_border'], 0.6) + text(x + 29, y + 40, i + 1, 29, p['lock_screen.key_label'], anchor='middle')
        s += rect(151, 453, 58, 59, 'url(#uiTray)', 10, p['lock_screen.key_border'], 0.6) + text(180, 494, '0', 29, p['lock_screen.key_label'], anchor='middle')
        s += text(78, 490, 'Cancel', 15, p['lock_screen.cancel_label'], anchor='middle') + glyph('back', 272, 475, 21, p['lock_screen.back_glyph']) + text(180, GESTURE_Y - 32, 'Emergency call', 17, p['lock_screen.emergency_label'], anchor='middle')
    s += gesture(theme=theme)
    return s

def incoming(theme=DEFAULT_THEME, locked=False):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + status('Phone', True, theme=theme)
    popup_y = GESTURE_Y - 228
    if not locked:
        available_h = popup_y - STATUS_H
        s += f"""<defs><clipPath id="incomingClientClip">{rect(0, 0, W, available_h, 'white')}</clipPath></defs>"""
        s += f'<g transform="translate(0 {STATUS_H})"><g clip-path="url(#incomingClientClip)">{email_content(available_h, theme=theme)}</g></g>'
    else:
        s += text(180, 142, '9:41', 60, p['incoming.clock'], anchor='middle')
    s += rect(0, popup_y, W, 228, 'url(#uiDark)', 15, p['incoming.panel_border'], 0.7) + rect(0, GESTURE_Y - 18, W, 18, p['incoming.panel_footer'])
    s += icon('contacts', 18, popup_y + 22, 57) + text(89, popup_y + 46, 'Mara Ellis', 28, p['incoming.caller']) + text(89, popup_y + 72, 'Mobile', 17, p['incoming.caller_type']) + text(89, popup_y + 99, 'Incoming call', 15, p['incoming.state'])
    s += rect(17, popup_y + 134, 155, 65, 'url(#uiDestructiveAction)', 10, p['incoming.decline_border'], 0.6) + text(94, popup_y + 173, 'Decline', 21, p['incoming.decline_label'], 'bold', 'middle')
    s += rect(187, popup_y + 134, 155, 65, 'url(#uiSuccessAction)', 10, p['incoming.answer_border'], 0.6) + text(264, popup_y + 173, 'Answer', 21, p['incoming.answer_label'], 'bold', 'middle')
    return s + gesture(theme=theme)

def incoming_locked(theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + rect(0, 0, W, H, p['incoming_locked.scrim'], extra=('fill-opacity=".50"' if theme == 'fx0_gold' else 'fill-opacity=".2"'))
    s += text(180, 163, 'Mara Ellis', 33, p['incoming_locked.caller'], anchor='middle') + text(180, 195, 'Mobile', 17, p['incoming_locked.secondary'], anchor='middle')
    s += text(180, 225, 'Incoming call', 16, p['incoming_locked.secondary'], anchor='middle')
    s += circle(180, GESTURE_Y - 119, 64, p['incoming_locked.target_surface'], p['incoming_locked.target_border'], 0.7) + circle(180, GESTURE_Y - 88, 34, 'url(#uiDark)', p['incoming_locked.handle_border'], 1)
    s += icon('phone', 157, GESTURE_Y - 112, 46) + text(180, GESTURE_Y - 32, 'Drag up to answer', 16, p['incoming_locked.instruction'], anchor='middle')
    return s + gesture(theme=theme)

def ongoing_call(theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + status('Phone', True, theme=theme) + rect(0, STATUS_H, W, APP_H, p['ongoing_call.surface'])
    s += text(180, 90, 'Mara Ellis', 30, p['ongoing_call.caller'], anchor='middle') + text(180, 121, 'Mobile', 17, p['ongoing_call.caller_type'], anchor='middle')
    s += circle(180, 206, 67, p['ongoing_call.avatar_surface']) + circle(180, 184, 24, p['ongoing_call.avatar_figure']) + f'''<path d="M132 250v-8q0-38 48-38t48 38v8" fill="{p['ongoing_call.avatar_figure']}"/>'''
    s += text(180, 317, '03:24', 24, p['ongoing_call.timer'], anchor='middle')
    for x, gl, lab in [(29, 'speaker', 'Speaker'), (137, 'mic', 'Mute'), (245, 'plus', 'Add call')]:
        s += rect(x, 357, 86, 90, 'url(#uiDark)', 9, p['ongoing_call.control_border'], 0.7) + glyph(gl, x + 29, 373, 27, p['ongoing_call.control_glyph']) + text(x + 43, 432, lab, 14, p['ongoing_call.control_label'], anchor='middle')
    s += rect(22, GESTURE_Y - 112, 316, 65, 'url(#uiDestructiveAction)', 9, p['ongoing_call.end_border'], 0.6) + text(180, GESTURE_Y - 71, 'End call', 23, p['ongoing_call.end_label'], 'bold', 'middle')
    return s + gesture(theme=theme)

def boot(theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    s = wallpaper(theme) + rect(0, 0, W, H, p['boot.scrim'], extra='fill-opacity=".50"')
    s += brand_mark(144, 230 + BOTTOM_SHIFT / 2, 72, p['boot.mark'], theme=theme) + text(180, 393 + BOTTOM_SHIFT / 2, 'TAROT', 43, p['boot.wordmark'], 'normal', 'middle', family='Instrument Serif', extra='letter-spacing="6"')
    return s

def brand_mark(x, y, size=64, c=None, theme=DEFAULT_THEME):
    p = UI_ROLES[theme]
    c = p['brand.stroke'] if c is None else c
    s = f'<g transform="translate({x} {y}) scale({size / 64})" fill="none" stroke="{c}" stroke-width="1.6">'
    s += f'''<rect x="12" y="7" width="29" height="45" rx="4" transform="rotate(-17 26 49)"/><rect x="21" y="6" width="29" height="45" rx="4" transform="rotate(12 35 49)"/><rect x="18" y="5" width="29" height="45" rx="4" fill="{p['brand_mark.field']}"/><path d="m32.5 16 3.5 8 8 3.5-8 3.5-3.5 8-3.5-8-8-3.5 8-3.5Z"/><path d="M23 11h4M38 44h4"/>'''
    return s + '</g>'

def svg(body,theme=DEFAULT_THEME,w=W,h=H):
    return f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w*SCALE}" height="{h*SCALE}" viewBox="0 0 {w} {h}">{defs(theme)}<defs><clipPath id="outerClip">{rect(0,0,w,h,"white")}</clipPath></defs><g clip-path="url(#outerClip)">{body}</g></svg>'

# Render lazily with the selected theme carried through every nested UI layer.
# The reference screen retains the original Nocturne appearance.
screen_specs = {
    '01-card-view-fx0-gold': (overview, {}, DEFAULT_THEME),
    '02-card-stack': (overview, {'stack': True}, DEFAULT_THEME),
    '03-launcher': (launcher, {}, DEFAULT_THEME),
    '04-email': (full_app, {'kind': 'email'}, DEFAULT_THEME),
    '05-banner': (full_app, {'kind': 'email', 'banner': True}, DEFAULT_THEME),
    '06-dashboard': (full_app, {'kind': 'email', 'dashboard': True}, DEFAULT_THEME),
    '07-wave-launcher': (full_app, {'kind': 'calendar', 'wave': True}, DEFAULT_THEME),
    '08-device-menu': (full_app, {'kind': 'email', 'device_menu': True}, DEFAULT_THEME),
    '09-app-menu': (full_app, {'kind': 'email', 'app_menu': True}, DEFAULT_THEME),
    '10-just-type': (search_screen, {}, DEFAULT_THEME),
    '11-lock': (lock_screen, {}, DEFAULT_THEME),
    '12-pin': (lock_screen, {'pin': True}, DEFAULT_THEME),
    '13-incoming-call': (incoming, {}, DEFAULT_THEME),
    '14-active-call': (ongoing_call, {}, DEFAULT_THEME),
    '15-settings': (full_app, {'kind': 'settings'}, DEFAULT_THEME),
    '16-empty-card-view': (overview, {'empty': True}, DEFAULT_THEME),
    '17-heritage': (overview, {}, 'heritage'),
    '18-vellum': (overview, {}, 'vellum'),
    '19-messaging': (full_app, {'kind': 'messages'}, DEFAULT_THEME),
    '20-boot': (boot, {}, DEFAULT_THEME),
    '21-calendar': (full_app, {'kind': 'calendar'}, DEFAULT_THEME),
    '22-notification-rail': (notification_rail, {}, DEFAULT_THEME),
    '23-launch-pending': (launch_pending, {}, DEFAULT_THEME),
    '24-launch-failed': (launch_pending, {'failed': True}, DEFAULT_THEME),
    '25-locked-incoming-call': (incoming_locked, {}, DEFAULT_THEME),
    '26-nocturne-reference': (overview, {}, 'nocturne'),
}
titles = {'03-launcher':'Launcher','04-email':'Email','05-banner':'Email','06-dashboard':'Email','07-wave-launcher':'Calendar','08-device-menu':'Email','09-app-menu':'Email','10-just-type':'Just Type','13-incoming-call':'Phone','14-active-call':'Phone','15-settings':'Preferences','19-messaging':'Messaging','21-calendar':'Calendar','22-notification-rail':'Email'}
for name, (render, arguments, theme) in screen_specs.items():
    body = render(theme=theme, **arguments)
    if name != '20-boot':
        body += status(titles.get(name, 'Tarot'), name in titles, theme=theme)
    (SCREENS / f'{name}.svg').write_text(svg(body, theme))
# Remove only the superseded primary screen, including a copied stale raster.
for extension, folder in [('svg', SCREENS), ('png', OUT / 'screens' / 'png')]:
    (folder / f'01-card-view-nocturne.{extension}').unlink(missing_ok=True)

for theme in THEMES:
    (OUT / 'wallpapers' / f'{theme}.svg').write_text(svg(wallpaper(theme), theme))

p = UI_ROLES[DEFAULT_THEME]
mark = f'<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 64 64">{brand_mark(0,0,64,theme=DEFAULT_THEME)}</svg>'
(OUT / 'brand' / 'tarot-mark.svg').write_text(mark)
logo = '<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="280" viewBox="0 0 600 140">'
logo += brand_mark(6,23,94,theme=DEFAULT_THEME)
logo += text(128,102,'TAROT',83,p['brand.wordmark'],family='Instrument Serif',extra='letter-spacing="12"')
logo += '</svg>'
(OUT / 'brand' / 'tarot-wordmark.svg').write_text(logo)

components = rect(0,0,720,216,p['components.surface'])
for i,label in enumerate(['Default','Pressed','Disabled','Busy']):
    x = 18+i*174
    components += text(x+75,27,label,15,p['components.label'],'bold','middle')
    components += rect(x,43,150,42,p['components.pressed_surface'] if i==1 else (p['components.disabled_surface'] if i==2 else 'url(#uiButton)'),8,p['components.border'],.9)
    components += text(x+75,70,'Connecting...' if i==3 else 'Connect',16,p['components.pressed_label'] if i==1 else (p['components.disabled_label'] if i==2 else p['components.button_label']),'bold','middle')
    components += text(x+75,127,['Off','On','Pending','Unavailable'][i],15,p['components.label'],'bold','middle')
    components += rect(x+39,145,72,30,'url(#uiAccent)' if i==1 else p['components.switch_off_surface'],15,p['components.switch_border'],.8)
    knob=x+(94 if i==1 else 55)
    components += circle(knob,160,13,'url(#uiButton)' if i!=3 else p['components.disabled_knob'],p['components.knob_border'],.7)
    components += text(x+(61 if i==1 else 88),164,'ON' if i==1 else 'OFF',10,p['switch.on_label'] if i==1 else p['switch.off_label'],'bold','middle')
    if i==2:
        components += f'<path d="M{x+125} 154a8 8 0 1 1-6 13" fill="none" stroke="{p["components.pending_indicator"]}" stroke-width="2"/>'
    components += text(x+75,204,['Ready to change','Confirmed by service','Waiting for service','Control is unavailable'][i],11,p['components.label'],anchor='middle')
(OUT / 'components' / 'control-states.svg').write_text(svg(components,DEFAULT_THEME,720,216))
# Pictorial app icons retain their own original gradients and authored colors.
for name in ['phone','contacts','mail','calendar','launcher','web','messages','camera','photos','music','clock','notes','settings','files','terminal','store']:
    (OUT / 'icons' / f'{name}.svg').write_text(svg(icon(name,0,0,64),DEFAULT_THEME,64,64))
for name in ['search','chevron','back','plus','check','close','up','wifi','bluetooth','plane','speaker','mic','lock','refresh','trash','pencil','power','sun']:
    (OUT / 'glyphs' / f'{name}.svg').write_text(svg(glyph(name,2,2,24,p['glyph.default']),DEFAULT_THEME,28,28))

# These are proposed Tarot dimensions, intentionally distinct from measured historical values.
tokens={
 'schema_version':3,
 'active_theme':DEFAULT_THEME,
 'default_theme':DEFAULT_THEME,
 'status':'design proposal; calibrate against one captured webOS 2.x build before claiming conformance',
 'reference':'Palm Pre 2 webOS 2.x phone interaction model',
 'device_profile':{
     'name':'LG Fx0','model':'LGL25','codename':'madai',
     'profile_status':'design target; hardware input and backend timing require device verification',
     'physical_home_button':{
         'status':'proposed binding, enabled only after verifying the actual input capability',
         'input_mapping':'resolve from verified input capability and observed device mapping; do not hardcode a numeric keycode',
         'short_press_from_app':'Card view',
         'short_press_from_card_view':'Launcher',
         'short_press_from_launcher':'Card view',
         'while_locked':'never unlock, authenticate, or expose protected cards',
         'touch_gesture_strip_retained':True
     }
 },
 'units':{'layout':'logical pixels','default_scale':SCALE,'output_px':[W*SCALE,H*SCALE],'canvas_dp':[W,H]},
 'geometry':{
     'status_bar_h':STATUS_H,'gesture_strip_h':GESTURE_H,
     'gesture_strip_rect':[0,GESTURE_Y,W,GESTURE_H],
     'app_rect':[0,STATUS_H,W,APP_H],'header_h':56,'command_bar_h':56,
     'list_row_min_h':51,'primary_action_touch_min':48,
     'compact_target_policy':'status, gesture and keyboard have equivalent accessible routes; do not silently extend hitboxes over clients',
     'card_scale':0.66,'inactive_card_scale':0.61,'card_radius_before_scale':11,
     'card_anchor':[61.2,CARD_Y],'card_client_size':[W,APP_H],
     'stack_preview_poses':[{'x':43,'y':124,'scale':0.64,'degrees':-5},{'x':62,'y':118,'scale':0.64,'degrees':0},{'x':81,'y':124,'scale':0.64,'degrees':5}],
     'quick_launch_rect':[3,611+BOTTOM_SHIFT,354,69],
     'quick_launch_slots':5,'quick_launch_icon_size':56,
     'launcher_columns':3,'launcher_icon_size':58,'launcher_row_pitch':96,'launcher_rows_visible':5,
     'banner_h':BANNER_H,'banner_accessible_h':48,
     'banner_rect':[0,GESTURE_Y-BANNER_H,W,BANNER_H],
     'dashboard_row_h':56,'dashboard_rect':[0,GESTURE_Y-DASHBOARD_H,W,DASHBOARD_H],
     'keyboard_h':KEYBOARD_H,'keyboard_rect':[0,KEYBOARD_Y,W,KEYBOARD_H],
     'search_results_viewport':[0,92,W,KEYBOARD_Y-92],
     'search_overflow_policy':'scroll further actions vertically; one New message action visible initially at full text size',
     'incoming_call_rect':[0,GESTURE_Y-228,W,228],
     'call_primary_action_h':65
 },
 'type':{'mockup_ui_family':'Lato','historical_target_family':'Prelude','brand_family':'Instrument Serif','status':15,'clock':16,'header':24,'body':19,'secondary':15,'metadata':13,'launcher_label':14,'font_match_pending':True},
 'semantic_palette':{'status':'#030505','icon_success':'#34733D','icon_destructive':'#9B302C','success_action':['#43793A','#2D592A'],'destructive_action':['#AC4538','#7E2B25']},
 'role_tokens':UI_ROLES[DEFAULT_THEME],
 'themes':{name:dict(metadata,roles=UI_ROLES[name]) for name,metadata in THEMES.items()},
 'theme_contract':{
     'scope':'shell-owned surfaces and cooperative sample app chrome; arbitrary client buffers retain their submitted pixels',
     'icon_policy':'pictorial app icons and browser content illustration retain their original colors',
     'gradient_policy':'themeable UI gradients have separate IDs from original icon gradients',
     'focus':{'on_dark':UI_ROLES[DEFAULT_THEME]['focus.on_dark'],'on_light':UI_ROLES[DEFAULT_THEME]['focus.on_light']},
     'status_background':'#030505',
     'compositing':{
         'launcher_scrim':{'color':'#211B12','opacity':0.5},
         'lock_scrim':{'color':'#211B12','opacity':0.5},
         'locked_call_scrim':{'color':'#211B12','opacity':0.5},
         'boot_scrim':{'color':'#211B12','opacity':0.5},
         'scope':'Fx0 Gold; other themes retain their original opacities'
     },
     'wallpaper_treatment':'original static engraved and foil linework, quiet center, restrained edge detail',
     'text_contrast':'Gold endpoint pairs audited; final text compositing and device rendering still require review'
 },
 'motion_ms':{'maximize':300,'minimize':350,'launch':400,'dismiss':300,'launcher':350,'quick_launch':350,'press_feedback':70,'reduced_motion_fade':100},
 'motion_status':'starting values from later LunaSysMgr config, not a captured Pre 2 timing match',
 'gesture_proposals':{'start_zone':'reserved strip only for shell navigation','tap_slop_dp':10,'axis_lock_dp':14,'back_min_dp':54,'home_flick_velocity_dp_s':700,'wave_dwell_ms':140,'wave_drag_min_dp':24,'reorder_hold_ms':420,'card_dismiss_distance_h':0.28,'card_dismiss_velocity_dp_s':900,'advanced_swipe_min_dp':240,'cancel_on_second_touch':True,'calibration_pending':True},
 'compositor_budget_proposal':{
     'status':'proposal; not measured on an LG Fx0 runtime',
     'preview_cache_max_bytes':33554432,
     'max_retained_previews':8,
     'cache_scope':'Tarot-owned cached previews only; excludes client surfaces and total process RSS',
     'live_preview_policy':'focused and visible neighboring cards only',
     'eviction_policy':'least recently used cached previews first under memory pressure',
     'blur':'precomputed shadows; no continuous full-screen blur',
     'render_cadence':'derive from the actual backend timing; no verified 60 Hz assumption',
     'rgba8_native_frame_bytes':W*SCALE*H*SCALE*4
 },
 'theme_invariants':['status position and black fill','card geometry','menu anchors','gesture semantics','notification placement','icon slot count','text contrast','security state'],
 'content_disclaimer':'Fictional contacts and app data. App interiors show cooperative design targets, not automatic GTK or Qt restyling.'
}
(OUT/'tokens'/'tarot.tokens.json').write_text(json.dumps(tokens,indent=2))
gold_theme = {
    'schema_version':1,
    'theme_key':'fx0_gold',
    'name':'Fx0 Gold',
    'active_theme':DEFAULT_THEME,
    'device_profile':{'name':'LG Fx0','model':'LGL25','codename':'madai'},
    'wallpaper':{key:value for key,value in THEMES['fx0_gold'].items() if key not in ['name','accent','focus']},
    'accent':THEMES['fx0_gold']['accent'],
    'focus':{'on_dark':UI_ROLES['fx0_gold']['focus.on_dark'],'on_light':UI_ROLES['fx0_gold']['focus.on_light']},
    'role_tokens':UI_ROLES['fx0_gold'],
    'semantic_palette':tokens['semantic_palette'],
    'contract':tokens['theme_contract'],
    'compositing':tokens['theme_contract']['compositing'],
    'geometry_source':'tarot.tokens.json; all Fx0 geometry and motion are preserved'
}
(OUT/'tokens'/'fx0-gold.theme.json').write_text(json.dumps(gold_theme,indent=2))
print('Wrote',len(screen_specs),'editable screens and',len(THEMES),'wallpapers to',OUT)
