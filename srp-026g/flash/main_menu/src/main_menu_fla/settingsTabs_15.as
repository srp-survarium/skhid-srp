package main_menu_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import scaleform.clik.controls.TileListSmoothR;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.FocusManager;
   import survarium.menu.Settings;
   
   public dynamic class settingsTabs_15 extends MovieClip
   {
      
      public var _ao_q:OptionStepperCyclic;
      
      public var _setter_static_allow_statistics:CheckBoxSkinned;
      
      public var _setter_static_show_quests:CheckBoxSkinned;
      
      public var _setter_invite:CheckBoxSkinned;
      
      public var _total_volume:SrvOptionSlider;
      
      public var _select_monitor:DropdownMenuSkinnedLabel;
      
      public var _setter_static_use_seed:CheckBoxSkinned;
      
      public var bottomContainer:MovieClip;
      
      public var _mic_sens:SrvOptionSlider;
      
      public var _sound_volume:SrvOptionSlider;
      
      public var _setter_static_minimap:CheckBoxSkinned;
      
      public var _only_friends:CheckBoxSkinned;
      
      public var _vsync:CheckBoxSkinned;
      
      public var _fullscreen:CheckBoxSkinned;
      
      public var _lighting_q:OptionStepperCyclic;
      
      public var keysChat:FilterInventoryList;
      
      public var _setter_inverse_mouse:CheckBoxSkinned;
      
      public var _hide_spam:CheckBoxSkinned;
      
      public var _ptt:CheckBoxSkinned;
      
      public var slider_gamma:SrvOptionSlider;
      
      public var _decorations:OptionStepperCyclic;
      
      public var button_default_video:SetingsButton;
      
      public var _fscr_resolution:OptionStepperCyclic;
      
      public var _use_mik:CheckBoxSkinned;
      
      public var _water_q:OptionStepperCyclic;
      
      public var _slider_mouse_sens:SrvOptionSlider;
      
      public var s_quest_propgress:TextField;
      
      public var _slider_quest_alpha:SrvOptionSlider;
      
      public var _no_in_batle:CheckBoxSkinned;
      
      public var _sound_сhat:SrvOptionSlider;
      
      public var _game_volume:SrvOptionSlider;
      
      public var _set_censure:CheckBoxSkinned;
      
      public var _shadow_q:OptionStepperCyclic;
      
      public var _post_process:OptionStepperCyclic;
      
      public var _geometry_q:OptionStepperCyclic;
      
      public var crossList:MovieClip;
      
      public var s_friends:TextField;
      
      public var screenAdjust:MovieClip;
      
      public var _texture_q:OptionStepperCyclic;
      
      public var _overall_graphics:OptionStepperCyclic;
      
      public var _setter_static_cross:CheckBoxSkinned;
      
      public var _slider_fov:SrvOptionSlider;
      
      public var _particples_q:OptionStepperCyclic;
      
      public var s_chat_keys:TextField;
      
      public var _filtering:OptionStepperCyclic;
      
      public var _antialias:OptionStepperCyclic;
      
      public var button_default_controls:SetingsButton;
      
      public var _setter_message:CheckBoxSkinned;
      
      public var s_voice_chat:TextField;
      
      public var s_sound_volume:TextField;
      
      public var button_optimal_video:SetingsButton;
      
      public var keysGame:FilterInventoryList;
      
      public var s_cross:TextField;
      
      public var _motion_blur:OptionStepperCyclic;
      
      public var _upload_speed:OptionStepperCyclic2;
      
      public var _setter_static_minimap_position:DropdownMenuSkinned;
      
      public var keysLobby:FilterInventoryList;
      
      public var s_lobby:TextField;
      
      public var __setPropDict:Dictionary = new Dictionary(true);
      
      public var _ignore:Boolean;
      
      public var _cross:Array;
      
      public var _object:Object;
      
      public var i:int;
      
      public var gamma_option:Sprite;
      
      public function settingsTabs_15()
      {
         super();
         addFrameScript(0,this.frame1,1,this.frame2,2,this.frame3,3,this.frame4);
      }
      
      public function setNewCross(param1:Event) : void
      {
         var _loc2_:TileListSmoothR = this.crossList.list;
         var _loc3_:* = _loc2_.getRendererAt(_loc2_.selectedIndex,_loc2_.scrollPosition);
         trace("set new cross:",_loc2_,_loc3_,_loc2_.selectedIndex,_loc2_.scrollPosition);
         this.showSelect(null);
      }
      
      public function showSelect(param1:Event = null) : *
      {
         if(this._ignore)
         {
            _ignore = false;
            return;
         }
         if((param1))
         {
            trace("showSelct",param1.target,param1.currentTarget);
         }
         else
         {
            trace("showSelct no event");
         }
      }
      
      public function onStageClick(param1:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.onStageClick);
      }
      
      public function crossListFO(param1:FocusEvent) : void
      {
         stage.addEventListener(MouseEvent.MOUSE_UP,this.onStageClick);
         if(param1.relatedObject == null || !this.crossList.contains(param1.relatedObject))
         {
            this.showSelect();
         }
      }
      
      public function callExternal(param1:Event) : void
      {
         ExternalInterface.call(param1.target.name);
      }
      
      public function fdsfsd(param1:Event) : void
      {
         if(!this.screenAdjust.visible)
         {
            parent.visible = true;
         }
      }
      
      public function showScreenAdjust(param1:ButtonEvent) : void
      {
         this.screenAdjust.btn_set.label = Settings.get_label("button_ok");
         var _loc2_:Point = localToGlobal(new Point(x,y));
         this.screenAdjust.visible = true;
         this.screenAdjust.setSize(stage.stageWidth,stage.stageHeight);
         parent.visible = false;
         stage.addChild(this.screenAdjust);
         FocusManager.setModalClip(this.screenAdjust);
      }
      
      internal function __setProp__setter_invite_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._setter_invite] == undefined || int(this.__setPropDict[this._setter_invite]) != 1)
         {
            this.__setPropDict[this._setter_invite] = 1;
            try
            {
               this._setter_invite["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._setter_invite.autoSize = "none";
            this._setter_invite.data = "";
            this._setter_invite.enabled = false;
            this._setter_invite.enableInitCallback = false;
            this._setter_invite.focusable = true;
            this._setter_invite.label = "";
            this._setter_invite.selected = false;
            this._setter_invite.visible = true;
            try
            {
               this._setter_invite["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__setter_message_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._setter_message] == undefined || int(this.__setPropDict[this._setter_message]) != 1)
         {
            this.__setPropDict[this._setter_message] = 1;
            try
            {
               this._setter_message["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._setter_message.autoSize = "none";
            this._setter_message.data = "";
            this._setter_message.enabled = false;
            this._setter_message.enableInitCallback = false;
            this._setter_message.focusable = false;
            this._setter_message.label = "";
            this._setter_message.selected = false;
            this._setter_message.visible = true;
            try
            {
               this._setter_message["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__only_friends_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._only_friends] == undefined || int(this.__setPropDict[this._only_friends]) != 1)
         {
            this.__setPropDict[this._only_friends] = 1;
            try
            {
               this._only_friends["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._only_friends.autoSize = "none";
            this._only_friends.data = "";
            this._only_friends.enabled = false;
            this._only_friends.enableInitCallback = false;
            this._only_friends.focusable = true;
            this._only_friends.label = "";
            this._only_friends.selected = false;
            this._only_friends.visible = true;
            try
            {
               this._only_friends["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__no_in_batle_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._no_in_batle] == undefined || int(this.__setPropDict[this._no_in_batle]) != 1)
         {
            this.__setPropDict[this._no_in_batle] = 1;
            try
            {
               this._no_in_batle["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._no_in_batle.autoSize = "none";
            this._no_in_batle.data = "";
            this._no_in_batle.enabled = false;
            this._no_in_batle.enableInitCallback = false;
            this._no_in_batle.focusable = true;
            this._no_in_batle.label = "";
            this._no_in_batle.selected = false;
            this._no_in_batle.visible = true;
            try
            {
               this._no_in_batle["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__hide_spam_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._hide_spam] == undefined || int(this.__setPropDict[this._hide_spam]) != 1)
         {
            this.__setPropDict[this._hide_spam] = 1;
            try
            {
               this._hide_spam["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._hide_spam.autoSize = "none";
            this._hide_spam.data = "";
            this._hide_spam.enabled = false;
            this._hide_spam.enableInitCallback = false;
            this._hide_spam.focusable = true;
            this._hide_spam.label = "";
            this._hide_spam.selected = false;
            this._hide_spam.visible = true;
            try
            {
               this._hide_spam["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__setter_static_cross_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._setter_static_cross] == undefined || int(this.__setPropDict[this._setter_static_cross]) != 1)
         {
            this.__setPropDict[this._setter_static_cross] = 1;
            try
            {
               this._setter_static_cross["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._setter_static_cross.autoSize = "none";
            this._setter_static_cross.data = "";
            this._setter_static_cross.enabled = false;
            this._setter_static_cross.enableInitCallback = false;
            this._setter_static_cross.focusable = true;
            this._setter_static_cross.label = "";
            this._setter_static_cross.selected = false;
            this._setter_static_cross.visible = true;
            try
            {
               this._setter_static_cross["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__setter_static_minimap_position_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._setter_static_minimap_position] == undefined || int(this.__setPropDict[this._setter_static_minimap_position]) != 1)
         {
            this.__setPropDict[this._setter_static_minimap_position] = 1;
            try
            {
               this._setter_static_minimap_position["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._setter_static_minimap_position.autoSize = "none";
            this._setter_static_minimap_position.dropdown = "OptionScrollingList";
            this._setter_static_minimap_position.enabled = true;
            this._setter_static_minimap_position.enableInitCallback = false;
            this._setter_static_minimap_position.focusable = true;
            this._setter_static_minimap_position.itemRenderer = "OptionsListRenderer";
            this._setter_static_minimap_position.menuDirection = "down";
            this._setter_static_minimap_position.menuMargin = 0;
            this._setter_static_minimap_position.inspectableMenuOffset = {
               "top":4,
               "right":0,
               "bottom":0,
               "left":0
            };
            this._setter_static_minimap_position.inspectableMenuPadding = {
               "top":5,
               "right":1,
               "bottom":5,
               "left":1
            };
            this._setter_static_minimap_position.menuRowCount = 10;
            this._setter_static_minimap_position.menuRowsFixed = false;
            this._setter_static_minimap_position.menuWidth = 184;
            this._setter_static_minimap_position.menuWrapping = "normal";
            this._setter_static_minimap_position.scrollBar = "";
            this._setter_static_minimap_position.inspectableThumbOffset = {
               "top":0,
               "bottom":0
            };
            this._setter_static_minimap_position.tooltipRendererName = "";
            this._setter_static_minimap_position.visible = true;
            try
            {
               this._setter_static_minimap_position["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__setter_static_use_seed_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._setter_static_use_seed] == undefined || int(this.__setPropDict[this._setter_static_use_seed]) != 1)
         {
            this.__setPropDict[this._setter_static_use_seed] = 1;
            try
            {
               this._setter_static_use_seed["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._setter_static_use_seed.autoSize = "center";
            this._setter_static_use_seed.data = "";
            this._setter_static_use_seed.enabled = true;
            this._setter_static_use_seed.enableInitCallback = false;
            this._setter_static_use_seed.focusable = true;
            this._setter_static_use_seed.label = "defaultLabelIsSuperLongOrEvenBigger";
            this._setter_static_use_seed.selected = false;
            this._setter_static_use_seed.visible = true;
            try
            {
               this._setter_static_use_seed["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__setter_static_allow_statistics_settingsTabs_Layer2_0() : *
      {
         if(this.__setPropDict[this._setter_static_allow_statistics] == undefined || int(this.__setPropDict[this._setter_static_allow_statistics]) != 1)
         {
            this.__setPropDict[this._setter_static_allow_statistics] = 1;
            try
            {
               this._setter_static_allow_statistics["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._setter_static_allow_statistics.autoSize = "center";
            this._setter_static_allow_statistics.data = "";
            this._setter_static_allow_statistics.enabled = true;
            this._setter_static_allow_statistics.enableInitCallback = false;
            this._setter_static_allow_statistics.focusable = true;
            this._setter_static_allow_statistics.label = "defaultLabelIsSuperLongOrEvenBigger";
            this._setter_static_allow_statistics.selected = false;
            this._setter_static_allow_statistics.visible = true;
            try
            {
               this._setter_static_allow_statistics["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_keysLobby_settingsTabs_Layer2_1() : *
      {
         if(this.__setPropDict[this.keysLobby] == undefined || int(this.__setPropDict[this.keysLobby]) != 2)
         {
            this.__setPropDict[this.keysLobby] = 2;
            try
            {
               this.keysLobby["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.keysLobby.columnWidth = 279;
            this.keysLobby.direction = "vertical";
            this.keysLobby.enabled = true;
            this.keysLobby.enableInitCallback = false;
            this.keysLobby.externalColumnCount = 0;
            this.keysLobby.focusable = true;
            this.keysLobby.itemRendererName = "SrvBindKeyListRender";
            this.keysLobby.itemRendererInstanceName = "";
            this.keysLobby.margin = 0;
            this.keysLobby.noIndexOnLostFocus = true;
            this.keysLobby.inspectablePadding = {
               "top":0,
               "right":0,
               "bottom":0,
               "left":0
            };
            this.keysLobby.rowHeight = 30;
            this.keysLobby.scrollBar = "";
            this.keysLobby.visible = true;
            this.keysLobby.wrapping = "normal";
            try
            {
               this.keysLobby["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_keysChat_settingsTabs_Layer2_1() : *
      {
         if(this.__setPropDict[this.keysChat] == undefined || int(this.__setPropDict[this.keysChat]) != 2)
         {
            this.__setPropDict[this.keysChat] = 2;
            try
            {
               this.keysChat["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.keysChat.columnWidth = 279;
            this.keysChat.direction = "vertical";
            this.keysChat.enabled = true;
            this.keysChat.enableInitCallback = false;
            this.keysChat.externalColumnCount = 0;
            this.keysChat.focusable = true;
            this.keysChat.itemRendererName = "SrvBindKeyListRender";
            this.keysChat.itemRendererInstanceName = "";
            this.keysChat.margin = 0;
            this.keysChat.noIndexOnLostFocus = true;
            this.keysChat.inspectablePadding = {
               "top":0,
               "right":0,
               "bottom":0,
               "left":0
            };
            this.keysChat.rowHeight = 30;
            this.keysChat.scrollBar = "";
            this.keysChat.visible = true;
            this.keysChat.wrapping = "normal";
            try
            {
               this.keysChat["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_keysGame_settingsTabs_Layer2_1() : *
      {
         if(this.__setPropDict[this.keysGame] == undefined || int(this.__setPropDict[this.keysGame]) != 2)
         {
            this.__setPropDict[this.keysGame] = 2;
            try
            {
               this.keysGame["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.keysGame.columnWidth = 270;
            this.keysGame.direction = "vertical";
            this.keysGame.enabled = true;
            this.keysGame.enableInitCallback = false;
            this.keysGame.externalColumnCount = 0;
            this.keysGame.focusable = true;
            this.keysGame.itemRendererName = "SrvBindKeyListRender";
            this.keysGame.itemRendererInstanceName = "";
            this.keysGame.margin = 0;
            this.keysGame.noIndexOnLostFocus = true;
            this.keysGame.inspectablePadding = {
               "top":0,
               "right":0,
               "bottom":0,
               "left":0
            };
            this.keysGame.rowHeight = 30;
            this.keysGame.scrollBar = "SrvScrollBar";
            this.keysGame.visible = true;
            this.keysGame.wrapping = "normal";
            try
            {
               this.keysGame["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__fscr_resolution_settingsTabs_Layer2_2() : *
      {
         if(this.__setPropDict[this._fscr_resolution] == undefined || int(this.__setPropDict[this._fscr_resolution]) != 3)
         {
            this.__setPropDict[this._fscr_resolution] = 3;
            try
            {
               this._fscr_resolution["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._fscr_resolution.cyclic = true;
            this._fscr_resolution.enabled = true;
            this._fscr_resolution.enableInitCallback = false;
            this._fscr_resolution.focusable = true;
            this._fscr_resolution.visible = true;
            try
            {
               this._fscr_resolution["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_button_optimal_video_settingsTabs_Layer2_2() : *
      {
         if(this.__setPropDict[this.button_optimal_video] == undefined || int(this.__setPropDict[this.button_optimal_video]) != 3)
         {
            this.__setPropDict[this.button_optimal_video] = 3;
            try
            {
               this.button_optimal_video["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.button_optimal_video.autoRepeat = false;
            this.button_optimal_video.autoSize = "none";
            this.button_optimal_video.data = "";
            this.button_optimal_video.enabled = false;
            this.button_optimal_video.enableInitCallback = false;
            this.button_optimal_video.focusable = true;
            this.button_optimal_video.label = "";
            this.button_optimal_video.selected = false;
            this.button_optimal_video.toggle = false;
            this.button_optimal_video.visible = true;
            try
            {
               this.button_optimal_video["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__select_monitor_settingsTabs_Layer2_2() : *
      {
         if(this.__setPropDict[this._select_monitor] == undefined || int(this.__setPropDict[this._select_monitor]) != 3)
         {
            this.__setPropDict[this._select_monitor] = 3;
            try
            {
               this._select_monitor["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._select_monitor.autoSize = "none";
            this._select_monitor.dropdown = "OptionScrollingList";
            this._select_monitor.enabled = true;
            this._select_monitor.enableInitCallback = false;
            this._select_monitor.focusable = true;
            this._select_monitor.itemRenderer = "OptionsListRenderer";
            this._select_monitor.menuDirection = "down";
            this._select_monitor.menuMargin = 0;
            this._select_monitor.inspectableMenuOffset = {
               "top":4,
               "right":0,
               "bottom":0,
               "left":162
            };
            this._select_monitor.inspectableMenuPadding = {
               "top":5,
               "right":1,
               "bottom":5,
               "left":1
            };
            this._select_monitor.menuRowCount = 10;
            this._select_monitor.menuRowsFixed = false;
            this._select_monitor.menuWidth = 184;
            this._select_monitor.menuWrapping = "normal";
            this._select_monitor.scrollBar = "";
            this._select_monitor.inspectableThumbOffset = {
               "top":0,
               "bottom":0
            };
            this._select_monitor.tooltipRendererName = "";
            this._select_monitor.visible = true;
            try
            {
               this._select_monitor["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__use_mik_settingsTabs_Layer2_3() : *
      {
         if(this.__setPropDict[this._use_mik] == undefined || int(this.__setPropDict[this._use_mik]) != 4)
         {
            this.__setPropDict[this._use_mik] = 4;
            try
            {
               this._use_mik["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._use_mik.autoSize = "none";
            this._use_mik.data = "";
            this._use_mik.enabled = false;
            this._use_mik.enableInitCallback = false;
            this._use_mik.focusable = true;
            this._use_mik.label = "";
            this._use_mik.selected = false;
            this._use_mik.visible = true;
            try
            {
               this._use_mik["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp__ptt_settingsTabs_Layer2_3() : *
      {
         if(this.__setPropDict[this._ptt] == undefined || int(this.__setPropDict[this._ptt]) != 4)
         {
            this.__setPropDict[this._ptt] = 4;
            try
            {
               this._ptt["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this._ptt.autoSize = "none";
            this._ptt.data = "";
            this._ptt.enabled = false;
            this._ptt.enableInitCallback = false;
            this._ptt.focusable = true;
            this._ptt.label = "";
            this._ptt.selected = false;
            this._ptt.visible = true;
            try
            {
               this._ptt["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function frame1() : *
      {
         this.__setProp__setter_static_allow_statistics_settingsTabs_Layer2_0();
         this.__setProp__setter_static_use_seed_settingsTabs_Layer2_0();
         this.__setProp__setter_static_minimap_position_settingsTabs_Layer2_0();
         this.__setProp__setter_static_cross_settingsTabs_Layer2_0();
         this.__setProp__hide_spam_settingsTabs_Layer2_0();
         this.__setProp__no_in_batle_settingsTabs_Layer2_0();
         this.__setProp__only_friends_settingsTabs_Layer2_0();
         this.__setProp__setter_message_settingsTabs_Layer2_0();
         this.__setProp__setter_invite_settingsTabs_Layer2_0();
         stop();
         _ignore = false;
         _cross = new Array();
         i = 0;
         while(this.i < 10)
         {
            ++this.i;
         }
      }
      
      internal function frame2() : *
      {
         this.__setProp_keysGame_settingsTabs_Layer2_1();
         this.__setProp_keysChat_settingsTabs_Layer2_1();
         this.__setProp_keysLobby_settingsTabs_Layer2_1();
         this.button_default_controls.addEventListener(ButtonEvent.CLICK,this.callExternal,false,0,true);
      }
      
      internal function frame3() : *
      {
         this.__setProp__select_monitor_settingsTabs_Layer2_2();
         this.__setProp_button_optimal_video_settingsTabs_Layer2_2();
         this.__setProp__fscr_resolution_settingsTabs_Layer2_2();
         gamma_option = this.slider_gamma;
         trace("GAAMMMMA:",this.slider_gamma);
         this.screenAdjust.visible = false;
         this.screenAdjust.addEventListener(Event.REMOVED_FROM_STAGE,this.fdsfsd);
         this.button_optimal_video.addEventListener(ButtonEvent.CLICK,this.callExternal,false,0,true);
         this.button_default_video.addEventListener(ButtonEvent.CLICK,this.callExternal,false,0,true);
      }
      
      internal function frame4() : *
      {
         this.__setProp__ptt_settingsTabs_Layer2_3();
         this.__setProp__use_mik_settingsTabs_Layer2_3();
         this._game_volume.slider.enabled = false;
         this._game_volume.mouseEnabled = false;
         this._game_volume.alpha = 0.5;
         this._sound_volume.slider.enabled = false;
         this._sound_volume.mouseEnabled = false;
         this._sound_volume.alpha = 0.5;
         this._sound_сhat.slider.enabled = false;
         this._sound_сhat.mouseEnabled = false;
         this._sound_сhat.alpha = 0.5;
         this._sound_сhat.mouseChildren = this._sound_volume.mouseChildren = this._game_volume.mouseChildren = false;
         this._mic_sens.slider.enabled = false;
         this._mic_sens.mouseEnabled = false;
         this._mic_sens.alpha = 0.5;
      }
   }
}

