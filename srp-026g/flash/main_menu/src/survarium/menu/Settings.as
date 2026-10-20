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
   import survarium.controls.Message;
   
   public class Settings extends UIComponent
   {
      
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
      
      public function Settings()
      {
         super();
      }
      
      public static function get_label(param1:String) : String
      {
         if(!_labels_dict[param1])
         {
            return "#" + param1;
         }
         return _labels_dict[param1];
      }
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1);
      }
      
      override public function set y(param1:Number) : void
      {
         super.y = int(param1);
      }
      
      public function setDataProvider(param1:uint, param2:uint, param3:Array) : void
      {
         if(!param3)
         {
            return;
         }
         var _loc4_:* = this.settings_content[this._element_map[param1][param2]];
         _loc4_.dataProvider = new DataProvider(param3);
      }
      
      public function setValue(param1:uint, param2:uint, param3:Number, param4:Boolean = true) : void
      {
         var _loc5_:*;
         trace("setValue");
         this._content_values[param1][param2] = param3;
         if(param4)
         {
            _ignoreChange = true;
            _loc5_ = this.settings_content[this._element_map[param1][param2]];
            this._elements[_loc5_] = param3;
         }
         this.update_value(param1,param2,param4);
         addEventListener(Event.RENDER,this.endRender);
      }
      
      public function setValues(param1:uint, param2:Array, param3:Boolean = true) : void
      {
         if(!param2)
         {
            return;
         }
         var _loc4_:uint = 0;
         var _loc5_:*;
         while(_loc4_ < param2.length)
         {
            this._content_values[param1][param2[_loc4_].id] = param2[_loc4_].value;
            if(param3)
            {
               _loc5_ = this.settings_content[this._element_map[param1][param2[_loc4_].id]];
               this._elements[_loc5_] = param2[_loc4_].value;
            }
            _loc4_++;
         }
         if(this.tabs.selectedIndex == param1)
         {
            this.update_values(param1);
         }
      }
      
      public function setKeybindings(param1:Array) : void
      {
         _keys = param1;
         var _loc2_:uint = 0;
         while(_loc2_ < this._keys.length)
         {
            this._keys[_loc2_].defKey = this._keys[_loc2_].key;
            _loc2_++;
         }
      }
      
      public function setKeybind(param1:uint, param2:String, param3:Boolean) : void
      {
         var _loc4_:uint;
         if(this._keys)
         {
            _loc4_ = 0;
            while(_loc4_ < this._keys.length)
            {
               if(this._keys[_loc4_].action_id == param1)
               {
                  if(param3)
                  {
                     this._keys[_loc4_].defKey = param2;
                  }
                  else
                  {
                     this._keys[_loc4_].enter = true;
                  }
                  this._keys[_loc4_].key = param2;
               }
               _loc4_++;
            }
         }
         if(!param3)
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
      
      public function selectIndex(param1:uint) : void
      {
         this.tabs.selectedIndex = param1;
      }
      
      public function setSettings(param1:uint, param2:Array) : void
      {
         this._content[param1] = param2;
         if(param1 == this.tabs.selectedIndex)
         {
            this.create_labels(this.tabs.selectedIndex);
            this.create_content(this._content[this.tabs.selectedIndex]);
         }
      }
      
      public function hide() : void
      {
         var _loc1_:Object;
         if(this.btn_accept.enabled)
         {
            _loc1_ = {
               text:_labels_dict["apply_changes_msg"],
               selectedIndex:this._lastIndex,
               onNo:this.contre_pass_hide,
               onYes:this.contre_pass_hide,
               label_ok:_labels_dict["button_ok"],
               label_cancel:_labels_dict["button_cancel"]
            };
            Message.showMessage("applySettings",_loc1_,true);
         }
         else
         {
            visible = false;
         }
      }
      
      public function setLabels(param1:Array) : void
      {
         _labels_dict = new Dictionary();
         _labels = param1;
         var _loc2_:Array = new Array();
         var _loc3_:int = 1;
         var _loc4_:int = 0;
         var _loc5_:int;
         while(_loc4_ < param1.length)
         {
            trace("setLabels::",param1[_loc4_].name,param1[_loc4_].label);
            _labels_dict[param1[_loc4_].name] = param1[_loc4_].label;
            if(param1[_loc4_].name == "settings_label")
            {
               this.textFieldLabel.text = param1[_loc4_].label;
            }
            else if(param1[_loc4_].name == "button_apply")
            {
               this.btn_accept.label = param1[_loc4_].label;
            }
            else if(param1[_loc4_].name == "button_exit")
            {
               _exitLabel = param1[_loc4_].label;
               this.btn_cancel.label = param1[_loc4_].label;
            }
            else if(param1[_loc4_].name == "button_cancel")
            {
               _cancelLabel = param1[_loc4_].label;
               this.btn_cancel.label = param1[_loc4_].label;
            }
            else
            {
               _loc5_ = this._tabs_map.indexOf(param1[_loc4_].name);
               if(_loc5_ != -1)
               {
                  _loc2_[_loc5_] = param1[_loc4_].label;
                  _loc3_ += 1;
               }
            }
            _loc4_++;
         }
         this.tabs.dataProvider = new DataProvider(_loc2_);
         this.tabs.selectedIndex = 0;
         this.create_labels(this.tabs.selectedIndex == -1 ? 0 : (this.tabs.selectedIndex));
      }
      
      public function create_labels(param1:uint) : void
      {
         if(!this._labels_map[param1])
         {
            return;
         }
         var _loc2_:int = 0;
         var _loc3_:int;
         var _loc4_:Object;
         while(_loc2_ < this._labels_map[param1].length)
         {
            for(_loc3_ = 0; _loc3_ < this._labels.length; _loc3_++)
            {
               if(this._labels[_loc3_].name == this._labels_map[param1][_loc2_])
               {
                  _loc4_ = this.settings_content[this._labels_map[param1][_loc2_]];
                  if(!_loc4_)
                  {
                     continue;
                  }
                  {
                     if(_loc4_ is TextField)
                     {
                        _loc4_.text = this._labels[_loc3_].label;
                     }
                     else if(_loc4_ is Button)
                     {
                        _loc4_.label = this._labels[_loc3_].label;
                     }
                  }
               }
            }
            _loc2_++;
         }
      }
      
      protected function applyKeys() : void
      {
         var _loc1_:uint;
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
            _loc1_ = 0;
            while(_loc1_ < this._keys.length)
            {
               if(this._keys[_loc1_].group_id == 0 && this.settings_content.keysLobby)
               {
                  this.settings_content.keysLobby.dataProvider.push(this._keys[_loc1_]);
               }
               else if(this._keys[_loc1_].group_id == 1 && this.settings_content.keysChat)
               {
                  this.settings_content.keysChat.dataProvider.push(this._keys[_loc1_]);
               }
               else if(this.settings_content.keysGame)
               {
                  this.settings_content.keysGame.dataProvider.push(this._keys[_loc1_]);
               }
               _loc1_++;
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
      
      protected function onAccept(param1:Event) : void
      {
         var _loc2_:Object;
         var _loc3_:int;
         var _loc4_:Object;
         trace("ST:onAccept");
         for(_loc2_ in this._changed)
         {
            if(this._changed[_loc2_])
            {
               this._changed[_loc2_] = 0;
               _loc3_ = (this._element_map[this.tabs.selectedIndex].indexOf(_loc2_.name));
               if(_loc3_ != -1)
               {
                  if(_loc2_ is OptionSlider || _loc2_ is OptionStepper || _loc2_ is DropdownMenu)
                  {
                     _loc4_ = _loc2_.selectedIndex;
                  }
                  else if(_loc2_ is CheckBoxMultiline)
                  {
                     _loc4_ = uint(_loc2_.selected);
                  }
                  else if(false || _loc2_ is ListItemRenderer)
                  {
                     _loc4_ = _loc2_.data.key;
                  }
                  if(this._content_values[this.tabs.selectedIndex])
                  {
                     this._content_values[this.tabs.selectedIndex][_loc3_] = _loc4_;
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
      
      protected function onCancel(param1:Event) : void
      {
         var _loc2_:Object;
         if(!this._changes)
         {
            this.onExit(param1);
         }
         for(_loc2_ in this._changed)
         {
            this._changed[_loc2_] = 0;
         }
         this.btn_accept.enabled = false;
         this.btn_cancel.label = this._exitLabel;
         _changes = 0;
         ExternalInterface.call("cancel_changes",this.tabs.selectedIndex);
      }
      
      protected function onExit(param1:Event) : void
      {
         parent["action_select"].visible = true;
         this.hide();
      }
      
      override protected function configUI() : void
      {
         this.btn_accept.enabled = false;
         this.btn_cancel.label = this._exitLabel;
         var _loc1_:Graphics = this.bottomContainer.graphics;
         _loc1_.beginFill(0,0.7);
         _loc1_.drawRect(0,0,this.bottomContainer.width / this.bottomContainer.scaleX,this.bottomContainer.height / this.bottomContainer.scaleY);
         _loc1_.endFill();
         this.bottomContainer.tiledSprite.setTile(new HintLinePattern());
         this._labels_map[0] = ["s_friends","s_cross","s_chat","s_minimap","s_minimap_position","s_quest_propgress","s_seed_settings"];
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
         this.tabs.addEventListener(IndexEvent.INDEX_CHANGE,this.takeAction);
      }
      
      protected function create_content(param1:Array) : void
      {
         SrpFovSlider.apply(this);
         var _loc2_:Object;
         var _loc3_:uint;
         var _loc4_:int;
         var _loc5_:*;
         var _loc6_:Object;
         var _loc7_:Array;
         var _loc8_:int;
         var _loc9_:Object;
         var _loc10_:String;
         if(!param1)
         {
            return;
         }
         for(_loc2_ in this._elements)
         {
            delete this._elements[_loc2_];
         }
         _elements = new Dictionary();
         _loc3_ = (this.tabs.selectedIndex);
         _loc4_ = 0;
         while(_loc4_ < param1.length)
         {
            _loc5_ = this.settings_content[this._element_map[this.tabs.selectedIndex][_loc4_]];
            if(_loc5_)
            {
               _loc6_ = param1[_loc4_];
               this._setters[_loc5_] = _loc6_.setter;
               if(_loc6_)
               {
                  _loc5_.label = _loc6_.label;
               }
               if(param1[_loc4_].dataProvider && ((!(param1[_loc4_].dataProvider is Array)) || param1[_loc4_].dataProvider is Array && param1[_loc4_].dataProvider.length))
               {
                  if(_loc5_ is OptionSlider)
                  {
                     _loc5_.dataProvider = param1[_loc4_].dataProvider;
                  }
                  else
                  {
                     _loc7_ = new Array();
                     _loc8_ = 0;
                     while(_loc8_ < param1[_loc4_].dataProvider.length)
                     {
                        if(!(param1[_loc4_].dataProvider[_loc8_] is String))
                        {
                           _loc7_[_loc8_] = new Object();
                           _loc9_ = param1[_loc4_].dataProvider[_loc8_];
                           for(_loc10_ in _loc9_)
                           {
                              _loc7_[_loc8_][_loc10_] = _loc9_[_loc10_];
                           }
                        }
                        else
                        {
                           _loc7_[_loc8_] = param1[_loc4_].dataProvider[_loc8_];
                        }
                        _loc8_++;
                     }
                     if(_loc5_ is DropdownMenu)
                     {
                        _loc5_.enabled = _loc7_.length > 1;
                     }
                     _loc5_.dataProvider = new DataProvider(_loc7_);
                  }
               }
               else if(_loc6_ && _loc6_.data)
               {
                  _loc5_.data = _loc6_.data;
               }
               if(_loc5_ is DropdownMenu)
               {
                  _loc5_.addEventListener(ListEvent.INDEX_CHANGE,this.someOneChangedIndex);
               }
               else if(_loc5_ is TileList)
               {
                  _loc5_.addEventListener(Event.CHANGE,this.someOneChangedData);
               }
               else if(_loc5_ is OptionSlider || _loc5_ is OptionStepper)
               {
                  _loc5_.addEventListener(IndexEvent.INDEX_CHANGE,this.someOneChangedIndex);
               }
               else if(_loc5_ is CheckBoxMultiline)
               {
                  _loc5_.addEventListener(Event.SELECT,this.someOneSelected);
               }
               if(_loc5_ is UIComponent)
               {
                  _loc5_.invalidateData();
               }
            }
            _loc4_++;
         }
         SrpFovSlider.finish(this);
      }
      
      protected function someOneSelected(param1:Event) : void
      {
         var _loc2_:int;
         if(this._setters[param1.target] && !this._ignoreChange)
         {
            param1.target.selected = this._setters[param1.target](param1.target.selected);
            _loc2_ = (this._element_map[this.tabs.selectedIndex].indexOf(param1.target.name));
            this._content_values[this.tabs.selectedIndex][_loc2_] = param1.target.selectedIndex;
         }
         if(param1.target.selected != this._elements[param1.target])
         {
            if(this._changed[param1.target] != 1)
            {
               this._changed[param1.target] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[param1.target] != 0)
         {
            this._changed[param1.target] = 0;
            _changes -= 1;
         }
         this.checkButtons();
      }
      
      protected function someOneChangedIndex(param1:Event) : void
      {
         var _loc2_:int;
         if(this._setters[param1.target] && !this._ignoreChange)
         {
            param1.target.selectedIndex = this._setters[param1.target](param1.target.selectedIndex);
            _loc2_ = (this._element_map[this.tabs.selectedIndex].indexOf(param1.target.name));
            this._content_values[this.tabs.selectedIndex][_loc2_] = param1.target.selectedIndex;
         }
         if(param1.target.selectedIndex != this._elements[param1.target])
         {
            if(this._changed[param1.target] != 1)
            {
               this._changed[param1.target] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[param1.target] != 0)
         {
            this._changed[param1.target] = 0;
            _changes -= 1;
         }
         this.checkButtons();
      }
      
      protected function someOneChangedData(param1:Event) : void
      {
         if(this._setters[param1.target] && !this._ignoreChange)
         {
            param1.target.data.key = this._setters[param1.target](uint(param1.target.data.key));
         }
         if(param1.target.data.key != param1.target.data.defKey || param1.target.data.toggle != param1.target.data.defToggle)
         {
            if(this._changed[param1.target] != 1)
            {
               this._changed[param1.target] = 1;
               _changes += 1;
            }
         }
         else if(this._changed[param1.target] != 0)
         {
            this._changed[param1.target] = 0;
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
      
      protected function update_value(param1:uint, param2:uint, param3:Boolean = false) : void
      {
         var _loc4_:* = this.settings_content[this._element_map[param1][param2]];
         var _loc5_:Object;
         if(_loc4_ && this._content_values[param1][param2] != undefined)
         {
            _loc5_ = this._content_values[param1][param2];
            if(param3)
            {
               this._elements[_loc4_] = _loc5_;
            }
            if(_loc4_ is OptionSlider || _loc4_ is OptionStepper || _loc4_ is DropdownMenu)
            {
               if(param3 || this._changed[_loc4_] == undefined)
               {
                  this._changed[_loc4_] = 0;
               }
               if(param3 && _loc4_ is OptionSlider)
               {
                  _loc4_.defaultValue = _loc5_;
               }
               _loc4_.selectedIndex = _loc5_;
            }
            else if(_loc4_ is CheckBoxMultiline)
            {
               if(param3 || this._changed[_loc4_] == undefined)
               {
                  this._changed[_loc4_] = 0;
               }
               _loc4_.selected = _loc5_;
            }
            if(_loc4_ is UIComponent)
            {
               _loc4_.invalidateData();
            }
         }
      }
      
      protected function update_values(param1:uint, param2:Boolean = false) : void
      {
         if(!this._content_values[param1])
         {
            return;
         }
         var _loc3_:int = 0;
         while(_loc3_ < this._content_values[param1].length)
         {
            this.update_value(param1,_loc3_,param2);
            _loc3_++;
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
         this.checkButtons();
         this.settings_content.gotoAndStop(this.tabs.selectedIndex + 1);
         this.create_labels(this.tabs.selectedIndex);
         this.create_content(this._content[this.tabs.selectedIndex]);
         if(this.tabs.selectedIndex == 1)
         {
            this.applyKeys();
         }
         this.update_values(this.tabs.selectedIndex,true);
      }
      
      protected function takeAction(param1:IndexEvent) : void
      {
         var _loc2_:Object;
         ExternalInterface.call("options_tab_changed",uint(param1.index));
         if(this.btn_accept.enabled)
         {
            _loc2_ = {
               text:_labels_dict["apply_changes_msg"],
               selectedIndex:this._lastIndex,
               onNo:this.contre_pass,
               onYes:this.contre_pass,
               label_ok:_labels_dict["button_ok"],
               label_cancel:_labels_dict["button_cancel"]
            };
            Message.showMessage("applySettings",_loc2_,true);
         }
         else
         {
            this.settings_content.gotoAndStop(param1.index + 1);
         }
         this.create_labels(param1.index == -1 ? 0 : this.tabs.selectedIndex);
         this.create_content(this._content[param1.index == -1 ? 0 : this.tabs.selectedIndex]);
         _lastIndex = param1.index;
         if(param1.index == 1)
         {
            this.applyKeys();
         }
         _ignoreChange = true;
         this.update_values(param1.index,true);
         addEventListener(Event.RENDER,this.endRender);
      }
      
      protected function endRender(param1:Event) : void
      {
         _ignoreChange = false;
         removeEventListener(Event.RENDER,this.endRender);
      }
      
      protected function onRenderValues(param1:Event) : void
      {
         this.update_values(this.tabs.selectedIndex,true);
         removeEventListener(Event.ENTER_FRAME,this.onRenderValues);
      }
   }
}

