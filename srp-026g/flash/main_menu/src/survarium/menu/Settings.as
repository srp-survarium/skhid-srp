package survarium.menu
{
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.DropdownMenu;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.controls.OptionStepper;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.ListEvent;
   import survarium.buttons.CheckBoxMultiline;
   import survarium.constants.KeyBindGroup;
   import survarium.controls.Message;
   
   public class Settings extends UIComponent
   {
      
      // The tab of the key bindings (opening it applies the keys).
      private static const KEYS_TAB:int = 1;
      
      protected static var _labels_dict:Dictionary;
      
      protected var _ignoreChange:Boolean = false;
      
      protected var _lastIndex:uint = 0;
      
      protected var _changes:int = 0;
      
      protected var _changed:Dictionary = new Dictionary();
      
      protected var _elements:Dictionary = new Dictionary();
      
      protected var _setters:Dictionary = new Dictionary();
      
      protected var _keys:Array;
      
      protected var _content:Array = new Array();
      
      protected var _labels:Array;
      
      protected var _content_values:Array = [[],[],[],[]];
      
      protected var _labels_map:Array = new Array();
      
      protected var _element_map:Array = new Array();
      
      protected var _tabs_map:Array = ["gameplay_options_type","controllers_options_type","video_options_type","sound_options_type"];
      
      public var tabs:ButtonBar;
      
      public var textFieldLabel:TextField;
      
      public var settings_content:MovieClip;
      
      public var btn_cancel:Button;
      
      public var btn_accept:Button;
      
      public var btn_close:Button;
      
      public var bottomContainer:MovieClip;
      
      protected var _exitLabel:String = "_exitLabel";
      
      protected var _cancelLabel:String = "_cancelLabel";
      
      // The gameplay tab's Language choice (exe patch 79): a pick is a change
      // of the tab as its options are, saved by Apply.
      protected var _language:SrpLanguage;
      
      // The gameplay tab's Lobby choice (exe patch 81), a change of the tab as
      // the Language's.
      protected var _lobbyScene:SrpLobbyScene;
      
      public function Settings()
      {
         super();
      }
      
      public static function get_label(key:String) : String
      {
         if(!_labels_dict[key])
         {
            return "#" + key;
         }
         return _labels_dict[key];
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value);
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value);
      }
      
      public function setDataProvider(tabIndex:uint, elementId:uint, items:Array) : void
      {
         if(!items)
         {
            return;
         }
         var control:* = this.settings_content[this._element_map[tabIndex][elementId]];
         control.dataProvider = new DataProvider(items);
      }
      
      public function setValue(tabIndex:uint, elementId:uint, value:Number, isBaseline:Boolean = true) : void
      {
         var control:*;
         trace("setValue");
         this._content_values[tabIndex][elementId] = value;
         if(isBaseline)
         {
            _ignoreChange = true;
            control = this.settings_content[this._element_map[tabIndex][elementId]];
            this._elements[control] = value;
         }
         this.update_value(tabIndex,elementId,isBaseline);
         addEventListener(Event.RENDER,this.endRender);
      }
      
      public function setValues(tabIndex:uint, values:Array, isBaseline:Boolean = true) : void
      {
         if(!values)
         {
            return;
         }
         var i:uint = 0;
         var control:*;
         while(i < values.length)
         {
            this._content_values[tabIndex][values[i].id] = values[i].value;
            if(isBaseline)
            {
               control = this.settings_content[this._element_map[tabIndex][values[i].id]];
               this._elements[control] = values[i].value;
            }
            i++;
         }
         if(this.tabs.selectedIndex == tabIndex)
         {
            this.update_values(tabIndex);
         }
      }
      
      public function setKeybindings(keys:Array) : void
      {
         _keys = keys;
         var i:uint = 0;
         while(i < this._keys.length)
         {
            this._keys[i].defKey = this._keys[i].key;
            i++;
         }
      }
      
      public function setKeybind(actionId:uint, key:String, isDefault:Boolean) : void
      {
         var i:uint;
         if(this._keys)
         {
            i = 0;
            while(i < this._keys.length)
            {
               if(this._keys[i].action_id == actionId)
               {
                  if(isDefault)
                  {
                     this._keys[i].defKey = key;
                  }
                  else
                  {
                     this._keys[i].enter = true;
                  }
                  this._keys[i].key = key;
               }
               i++;
            }
         }
         if(!isDefault)
         {
            this.btn_cancel.label = this._cancelLabel;
            this.btn_accept.enabled = true;
            _changes += 1;
         }
         if(this.settings_content.keysLobby)
         {
            this.settings_content.keysLobby.invalidateData();
         }
         if(this.settings_content.keysLobby)
         {
            this.settings_content.keysChat.invalidateData();
         }
         if(this.settings_content.keysLobby)
         {
            this.settings_content.keysGame.invalidateData();
         }
      }
      
      // The exe's answer to the Language choice (root.srp_language): the saved
      // language is selected, so no pick is pending.
      public function setLanguage(values:Array) : void
      {
         this._language.answer(values);
         this.languageChanged(null);
      }
      
      // The exe's answer to the Lobby choice (root.srp_lobby_scene): the saved
      // lobby is selected, so no pick is pending.
      public function setLobbyScene(values:Array) : void
      {
         this._lobbyScene.answer(values);
         this.lobbySceneChanged(null);
      }
      
      public function selectIndex(tabIndex:uint) : void
      {
         this.tabs.selectedIndex = tabIndex;
      }
      
      public function setSettings(tabIndex:uint, settings:Array) : void
      {
         this._content[tabIndex] = settings;
         if(tabIndex == this.tabs.selectedIndex)
         {
            this.create_labels(this.tabs.selectedIndex);
            this.create_content(this._content[this.tabs.selectedIndex]);
         }
      }
      
      public function hide() : void
      {
         var messageData:Object;
         if(this.btn_accept.enabled)
         {
            messageData = {
               text:_labels_dict["apply_changes_msg"],
               selectedIndex:this._lastIndex,
               onNo:this.contre_pass_hide,
               onYes:this.contre_pass_hide,
               label_ok:_labels_dict["button_ok"],
               label_cancel:_labels_dict["button_cancel"]
            };
            Message.showMessage("applySettings",messageData,true);
         }
         else
         {
            visible = false;
         }
      }
      
      public function setLabels(labelList:Array) : void
      {
         _labels_dict = new Dictionary();
         _labels = labelList;
         var tabLabels:Array = new Array();
         var tabCount:int = 1;
         var i:int = 0;
         var tabIndex:int;
         while(i < labelList.length)
         {
            trace("setLabels::",labelList[i].name,labelList[i].label);
            _labels_dict[labelList[i].name] = labelList[i].label;
            if(labelList[i].name == "settings_label")
            {
               this.textFieldLabel.text = labelList[i].label;
            }
            else if(labelList[i].name == "button_apply")
            {
               this.btn_accept.label = labelList[i].label;
            }
            else if(labelList[i].name == "button_exit")
            {
               _exitLabel = labelList[i].label;
               this.btn_cancel.label = labelList[i].label;
            }
            else if(labelList[i].name == "button_cancel")
            {
               _cancelLabel = labelList[i].label;
               this.btn_cancel.label = labelList[i].label;
            }
            else
            {
               tabIndex = this._tabs_map.indexOf(labelList[i].name);
               if(tabIndex != -1)
               {
                  tabLabels[tabIndex] = labelList[i].label;
                  tabCount += 1;
               }
            }
            i++;
         }
         this.tabs.dataProvider = new DataProvider(tabLabels);
         this.tabs.selectedIndex = 0;
         this.create_labels(this.tabs.selectedIndex == -1 ? 0 : (this.tabs.selectedIndex));
      }
      
      public function create_labels(tabIndex:uint) : void
      {
         if(!this._labels_map[tabIndex])
         {
            return;
         }
         var i:int = 0;
         var j:int;
         var target:Object;
         while(i < this._labels_map[tabIndex].length)
         {
            for(j = 0; j < this._labels.length; j++)
            {
               if(this._labels[j].name == this._labels_map[tabIndex][i])
               {
                  target = this.settings_content[this._labels_map[tabIndex][i]];
                  if(!target)
                  {
                     continue;
                  }
                  {
                     if(target is TextField)
                     {
                        target.text = this._labels[j].label;
                     }
                     else if(target is Button)
                     {
                        target.label = this._labels[j].label;
                     }
                  }
               }
            }
            i++;
         }
      }
      
      protected function applyKeys() : void
      {
         var i:uint;
         if(this._keys)
         {
            if(this.settings_content.keysLobby)
            {
               this.settings_content.keysLobby.dataProvider = new DataProvider();
            }
            if(this.settings_content.keysChat)
            {
               this.settings_content.keysChat.dataProvider = new DataProvider();
            }
            if(this.settings_content.keysGame)
            {
               this.settings_content.keysGame.dataProvider = new DataProvider();
            }
            i = 0;
            while(i < this._keys.length)
            {
               if(this._keys[i].group_id == KeyBindGroup.LOBBY && this.settings_content.keysLobby)
               {
                  this.settings_content.keysLobby.dataProvider.push(this._keys[i]);
               }
               else if(this._keys[i].group_id == KeyBindGroup.CHAT && this.settings_content.keysChat)
               {
                  this.settings_content.keysChat.dataProvider.push(this._keys[i]);
               }
               else if(this.settings_content.keysGame)
               {
                  this.settings_content.keysGame.dataProvider.push(this._keys[i]);
               }
               i++;
            }
            if(this.settings_content.keysChat)
            {
               this.settings_content.keysChat.height = this.settings_content.keysChat.dataProvider.length * this.settings_content.keysChat.rowHeight;
            }
            if(this.settings_content.keysLobby)
            {
               this.settings_content.keysLobby.height = this.settings_content.keysLobby.dataProvider.length * this.settings_content.keysLobby.rowHeight;
            }
         }
      }
      
      protected function onAccept(event:Event) : void
      {
         var control:Object;
         var elementIndex:int;
         var newValue:Object;
         trace("ST:onAccept");
         for(control in this._changed)
         {
            if(this._changed[control])
            {
               this._changed[control] = 0;
               if(control == this._language.menu)
               {
                  this._language.save();
               }
               if(control == this._lobbyScene.stepper)
               {
                  this._lobbyScene.save();
               }
               elementIndex = (this._element_map[this.tabs.selectedIndex].indexOf(control.name));
               if(elementIndex != -1)
               {
                  if(control is OptionSlider || control is OptionStepper || control is DropdownMenu)
                  {
                     newValue = control.selectedIndex;
                  }
                  else if(control is CheckBoxMultiline)
                  {
                     newValue = uint(control.selected);
                  }
                  else if(false || control is ListItemRenderer)
                  {
                     newValue = control.data.key;
                  }
                  if(this._content_values[this.tabs.selectedIndex])
                  {
                     this._content_values[this.tabs.selectedIndex][elementIndex] = newValue;
                  }
               }
            }
         }
         _changes = 0;
         this.btn_accept.enabled = false;
         this.btn_cancel.label = this._exitLabel;
         if(this.tabs.selectedIndex != -1)
         {
            this.update_values(this.tabs.selectedIndex);
            ExternalInterface.call("accept_changes",this.tabs.selectedIndex);
         }
      }
      
      protected function onCancel(event:Event) : void
      {
         var control:Object;
         if(!this._changes)
         {
            this.onExit(event);
         }
         for(control in this._changed)
         {
            this._changed[control] = 0;
         }
         this._language.revert();
         this._lobbyScene.revert();
         this.btn_accept.enabled = false;
         this.btn_cancel.label = this._exitLabel;
         _changes = 0;
         ExternalInterface.call("cancel_changes",this.tabs.selectedIndex);
      }
      
      protected function onExit(event:Event) : void
      {
         parent["action_select"].visible = true;
         this.hide();
      }
      
      override protected function configUI() : void
      {
         this.btn_accept.enabled = false;
         this.btn_cancel.label = this._exitLabel;
         var g:Graphics = this.bottomContainer.graphics;
         g.beginFill(0,0.7);
         g.drawRect(0,0,this.bottomContainer.width / this.bottomContainer.scaleX,this.bottomContainer.height / this.bottomContainer.scaleY);
         g.endFill();
         this.bottomContainer.tiledSprite.setTile(new HintLinePattern());
         this._labels_map[0] = ["s_friends","s_cross","s_chat","s_minimap","s_minimap_position","s_quest_propgress"];
         this._labels_map[1] = ["s_lobby","s_chat_keys","s_game","button_default_controls"];
         this._labels_map[2] = ["graphic","button_default_video","button_optimal_video","adjust_gamma"];
         this._labels_map[3] = ["s_sound_volume","s_voice_chat"];
         _changes = 0;
         this.btn_close.addEventListener(ButtonEvent.CLICK,this.onExit);
         this.btn_cancel.addEventListener(ButtonEvent.CLICK,this.onCancel);
         this.btn_accept.addEventListener(ButtonEvent.CLICK,this.onAccept);
         this._element_map[0] = ["_setter_invite","_setter_message","_set_censure","_only_friends","_no_in_batle","_hide_spam","theCross","_setter_static_cross","_setter_static_minimap","_setter_static_minimap_position","_setter_static_show_quests","_slider_quest_alpha","_setter_static_use_seed","_upload_speed","_setter_kill_cross","_setter_static_allow_statistics"];
         this._element_map[1] = ["_setter_inverse_mouse","_slider_mouse_sens","_default_button","keysLobby","keysChat","keysGame"];
         this._element_map[2] = ["_select_monitor","_fscr_resolution","_fullscreen","_vsync","_antialias","_filtering","gamma_option","_slider_fov","_overall_graphics","_texture_q","_geometry_q","_shadow_q","_lighting_q","_water_q","_decorations","_post_process","_ao_q","_particples_q","_motion_blur"];
         this._element_map[3] = ["_total_volume","_game_volume","_sound_volume","_sound_сhat","_use_mik","_mic_sens","_ptt"];
         this._language = new SrpLanguage(this.settings_content);
         this._changed[this._language.menu] = 0;
         this._language.menu.addEventListener(ListEvent.INDEX_CHANGE,this.languageChanged);
         this._lobbyScene = new SrpLobbyScene(this.settings_content);
         this._changed[this._lobbyScene.stepper] = 0;
         this._lobbyScene.stepper.addEventListener(IndexEvent.INDEX_CHANGE,this.lobbySceneChanged);
         this.tabs.addEventListener(IndexEvent.INDEX_CHANGE,this.takeAction);
      }
      
      protected function create_content(settings:Array) : void
      {
         SrpFovSlider.apply(this);
         this._language.place();
         this._lobbyScene.place();
         var key:Object;
         var tabIndex:uint;
         var i:int;
         var control:*;
         var setting:Object;
         var items:Array;
         var j:int;
         var source:Object;
         var field:String;
         if(!settings)
         {
            return;
         }
         for(key in this._elements)
         {
            delete this._elements[key];
         }
         _elements = new Dictionary();
         tabIndex = (this.tabs.selectedIndex);
         i = 0;
         while(i < settings.length)
         {
            control = this.settings_content[this._element_map[this.tabs.selectedIndex][i]];
            if(control)
            {
               setting = settings[i];
               this._setters[control] = setting.setter;
               if(setting)
               {
                  control.label = setting.label;
               }
               if(settings[i].dataProvider && ((!(settings[i].dataProvider is Array)) || settings[i].dataProvider is Array && settings[i].dataProvider.length))
               {
                  if(control is OptionSlider)
                  {
                     control.dataProvider = settings[i].dataProvider;
                  }
                  else
                  {
                     items = new Array();
                     j = 0;
                     while(j < settings[i].dataProvider.length)
                     {
                        if(!(settings[i].dataProvider[j] is String))
                        {
                           items[j] = new Object();
                           source = settings[i].dataProvider[j];
                           for(field in source)
                           {
                              items[j][field] = source[field];
                           }
                        }
                        else
                        {
                           items[j] = settings[i].dataProvider[j];
                        }
                        j++;
                     }
                     if(control is DropdownMenu)
                     {
                        control.enabled = items.length > 1;
                     }
                     control.dataProvider = new DataProvider(items);
                  }
               }
               else if(setting && setting.data)
               {
                  control.data = setting.data;
               }
               if(control is DropdownMenu)
               {
                  control.addEventListener(ListEvent.INDEX_CHANGE,this.someOneChangedIndex);
               }
               else if(control is TileList)
               {
                  control.addEventListener(Event.CHANGE,this.someOneChangedData);
               }
               else if(control is OptionSlider || control is OptionStepper)
               {
                  control.addEventListener(IndexEvent.INDEX_CHANGE,this.someOneChangedIndex);
               }
               else if(control is CheckBoxMultiline)
               {
                  control.addEventListener(Event.SELECT,this.someOneSelected);
               }
               if(control is UIComponent)
               {
                  control.invalidateData();
               }
            }
            i++;
         }
         // "Hide chat in match" (option 4 of the gameplay tab; exe patch 58 gives it its
         // variable, docs/notes/match-hud-options.md): the timeline enables it where it makes
         // the checkbox (settingsTabs_15); again here, after the tab's settings are bound.
         var hideChat:UIComponent = this.settings_content["_no_in_batle"] as UIComponent;
         if(hideChat)
         {
            hideChat.enabled = true;
         }
         SrpFovSlider.finish(this);
      }
      
      protected function someOneSelected(event:Event) : void
      {
         var elementIndex:int;
         if(this._setters[event.target] && !this._ignoreChange)
         {
            event.target.selected = this._setters[event.target](event.target.selected);
            elementIndex = (this._element_map[this.tabs.selectedIndex].indexOf(event.target.name));
            this._content_values[this.tabs.selectedIndex][elementIndex] = event.target.selectedIndex;
         }
         if(event.target.selected != this._elements[event.target])
         {
            if(this._changed[event.target] != 1)
            {
               this._changed[event.target] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[event.target] != 0)
         {
            this._changed[event.target] = 0;
            _changes -= 1;
         }
         this.checkButtons();
      }
      
      protected function someOneChangedIndex(event:Event) : void
      {
         var elementIndex:int;
         if(this._setters[event.target] && !this._ignoreChange)
         {
            event.target.selectedIndex = this._setters[event.target](event.target.selectedIndex);
            elementIndex = (this._element_map[this.tabs.selectedIndex].indexOf(event.target.name));
            this._content_values[this.tabs.selectedIndex][elementIndex] = event.target.selectedIndex;
         }
         if(event.target.selectedIndex != this._elements[event.target])
         {
            if(this._changed[event.target] != 1)
            {
               this._changed[event.target] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[event.target] != 0)
         {
            this._changed[event.target] = 0;
            _changes -= 1;
         }
         this.checkButtons();
      }
      
      protected function someOneChangedData(event:Event) : void
      {
         if(this._setters[event.target] && !this._ignoreChange)
         {
            event.target.data.key = this._setters[event.target](uint(event.target.data.key));
         }
         if(event.target.data.key != event.target.data.defKey || event.target.data.toggle != event.target.data.defToggle)
         {
            if(this._changed[event.target] != 1)
            {
               this._changed[event.target] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[event.target] != 0)
         {
            this._changed[event.target] = 0;
            _changes -= 1;
         }
         this.checkButtons();
      }
      
      // A Language pick counts as the change of an option: it is pending while
      // the selected language is not the saved one.
      protected function languageChanged(event:Event) : void
      {
         var menu:DropdownMenu = this._language.menu;
         if(this._language.pending)
         {
            if(this._changed[menu] != 1)
            {
               this._changed[menu] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[menu] == 1)
         {
            this._changed[menu] = 0;
            _changes -= 1;
         }
         this.checkButtons();
      }
      
      // A Lobby pick counts as the change of an option, as a Language pick.
      protected function lobbySceneChanged(event:Event) : void
      {
         var stepper:OptionStepper = this._lobbyScene.stepper;
         if(this._lobbyScene.pending)
         {
            if(this._changed[stepper] != 1)
            {
               this._changed[stepper] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[stepper] == 1)
         {
            this._changed[stepper] = 0;
            _changes -= 1;
         }
         this.checkButtons();
      }
      
      protected function checkButtons() : void
      {
         if(!this._changes)
         {
            this.btn_accept.enabled = false;
            this.btn_cancel.label = this._exitLabel;
         }
         else
         {
            this.btn_accept.enabled = true;
            this.btn_cancel.label = this._cancelLabel;
         }
      }
      
      protected function update_value(tabIndex:uint, elementId:uint, isBaseline:Boolean = false) : void
      {
         var control:* = this.settings_content[this._element_map[tabIndex][elementId]];
         var value:Object;
         if(control && this._content_values[tabIndex][elementId] != undefined)
         {
            value = this._content_values[tabIndex][elementId];
            if(isBaseline)
            {
               this._elements[control] = value;
            }
            if(control is OptionSlider || control is OptionStepper || control is DropdownMenu)
            {
               if(isBaseline || this._changed[control] == undefined)
               {
                  this._changed[control] = 0;
               }
               if(isBaseline && control is OptionSlider)
               {
                  control.defaultValue = value;
               }
               control.selectedIndex = value;
            }
            else if(control is CheckBoxMultiline)
            {
               if(isBaseline || this._changed[control] == undefined)
               {
                  this._changed[control] = 0;
               }
               control.selected = value;
            }
            if(control is UIComponent)
            {
               control.invalidateData();
            }
         }
      }
      
      protected function update_values(tabIndex:uint, isBaseline:Boolean = false) : void
      {
         if(!this._content_values[tabIndex])
         {
            return;
         }
         var i:int = 0;
         while(i < this._content_values[tabIndex].length)
         {
            this.update_value(tabIndex,i,isBaseline);
            i++;
         }
      }
      
      protected function contre_pass_hide() : void
      {
         this.contre_pass();
         visible = false;
      }
      
      protected function contre_pass() : void
      {
         _changes = 0;
         this._changed[this._language.menu] = 0;
         this._language.revert();
         this._changed[this._lobbyScene.stepper] = 0;
         this._lobbyScene.revert();
         this.checkButtons();
         this.settings_content.gotoAndStop(this.tabs.selectedIndex + 1);
         this.create_labels(this.tabs.selectedIndex);
         this.create_content(this._content[this.tabs.selectedIndex]);
         if(this.tabs.selectedIndex == KEYS_TAB)
         {
            this.applyKeys();
         }
         this.update_values(this.tabs.selectedIndex,true);
      }
      
      protected function takeAction(event:IndexEvent) : void
      {
         var messageData:Object;
         ExternalInterface.call("options_tab_changed",uint(event.index));
         if(this.btn_accept.enabled)
         {
            messageData = {
               text:_labels_dict["apply_changes_msg"],
               selectedIndex:this._lastIndex,
               onNo:this.contre_pass,
               onYes:this.contre_pass,
               label_ok:_labels_dict["button_ok"],
               label_cancel:_labels_dict["button_cancel"]
            };
            Message.showMessage("applySettings",messageData,true);
         }
         else
         {
            this.settings_content.gotoAndStop(event.index + 1);
         }
         this.create_labels(event.index == -1 ? 0 : this.tabs.selectedIndex);
         this.create_content(this._content[event.index == -1 ? 0 : this.tabs.selectedIndex]);
         _lastIndex = event.index;
         if(event.index == KEYS_TAB)
         {
            this.applyKeys();
         }
         _ignoreChange = true;
         this.update_values(event.index,true);
         addEventListener(Event.RENDER,this.endRender);
      }
      
      protected function endRender(event:Event) : void
      {
         _ignoreChange = false;
         removeEventListener(Event.RENDER,this.endRender);
      }
      
      protected function onRenderValues(event:Event) : void
      {
         this.update_values(this.tabs.selectedIndex,true);
         removeEventListener(Event.ENTER_FRAME,this.onRenderValues);
      }
   }
}

