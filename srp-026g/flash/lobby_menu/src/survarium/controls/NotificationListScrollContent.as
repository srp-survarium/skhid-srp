package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.InteractiveObject;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;
   import flash.utils.Timer;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.layout.LayoutData;
   import survarium.Util;
   import survarium.data.LocalizationData;
   import survarium.constants.ImportantMessageType;
   import survarium.constants.NotificationType;
   
   public class NotificationListScrollContent extends MovieClip
   {
      
      public static const HISTORY_EXTRA:int = 100;
      
      public static const REPLAY_START:uint = 2200000000;

      // srp's ask to watch a match's replay in this game, as a spectator (a replay session, the
      // lobby server's REPLAY_WATCH_BASE, docs/notes/replay-stream.md): the player of
      // victory_screen_request_player_data is 0xC0000000 | place << 16 | seconds, the place in
      // view first (0xFF anyone's) and the match time to start at. This one: from the start, anyone.
      public static const REPLAY_WATCH_ALL:uint = 0xC0FF0000;

      // The asker's own deaths (the lobby server's REPLAY_WATCH_MY_DEATHS, no fields): it finds the
      // asker's place in the recording and starts at the first death less 4 s.
      public static const REPLAY_WATCH_MY_DEATHS:uint = 0xC1000000;
      
      public static const REPLAY_POLLS:int = 30;
      
      public static const REPLAY_PREPARE_MS:int = 180000;
      
      // srp's replay prompt: its state (_rpState).
      private static const RP_IDLE:int = 0;
      
      // asking the match server, polling
      private static const RP_ASKING:int = 1;
      
      // a final message is shown
      private static const RP_ENDED:int = 2;
      
      // The prompt's texts (dictionary keys; rpShow); RP_KEY_NONE is no text shown.
      private static const RP_KEY_NONE:String = null;
      
      private static const RP_KEY_WATCH:String = "st_srp_replay_watch";

      private static const RP_KEY_WATCHING:String = "st_srp_replay_watching";

      // a session's `preparing` (the match server reads the recording)
      private static const RP_KEY_OPENING:String = "st_srp_replay_opening";
      
      private static const RP_KEY_CLOSE:String = "st_srp_replay_close";
      
      private static const RP_KEY_ASKING:String = "st_srp_replay_asking";
      
      // the final answers (rpFinalKey), by the server's status
      private static const RP_FINALS:Object = {
         "gone":"st_srp_replay_gone",
         "not_yours":"st_srp_replay_not_yours",
         "old_game":"st_srp_replay_old_game",
         "incompatible":"st_srp_replay_incompatible",
         "busy":"st_srp_replay_busy",
         "failed":"st_srp_replay_failed",
         "off":"st_srp_replay_off"
      };
      
      private static const RP_KEY_NO_ANSWER:String = "st_srp_replay_no_answer";
      
      private static const RP_KEY_STILL_PREPARING:String = "st_srp_replay_still_preparing";
      
      // A History row's chip: what the last answer said of its match's replay, by the server's
      // status (no chip before an answer, nor for one that says nothing of the replay).
      private static const RP_CHIPS:Object = {
         "watching":"st_srp_replay_chip_ready",
         "preparing":"st_srp_replay_chip_preparing",
         "incompatible":"st_srp_replay_chip_old",
         "gone":"st_srp_replay_chip_gone"
      };

      // The chip's colour by its text: ready green, preparing amber, the others grey.
      private static const RP_CHIP_COLOURS:Object = {
         "st_srp_replay_chip_ready":0x5FA65A,
         "st_srp_replay_chip_preparing":0xC8962E
      };

      private static const RP_CHIP_GREY:uint = 0x6E7378;

      // a row's match number (`#` and the 8 hex digits the server's rows and logs use), grey
      private static const RP_NUMBER_GREY:uint = 0x8A9096;

      private static const RP_KEY_NUMBER_BAD:String = "st_srp_replay_number_bad";
      
      // meaning unknown: onShowQuestMessage shows the quest message only for a notification of this type
      private static const QUEST_MESSAGE_TYPE:int = 3;
      
      public static var replayHolder:NotificationListScrollContent;

      // Told (match 8 hex, status) of each answer that sets a chip: the Watch window's cards follow.
      public static var chipListener:Function;
      
      public var matchCalback:Function;

      // The lobby's root, told by it: History is on the transient layer while open
      // (docs/notes/transient-windows.md), so its parent is not the lobby then.
      public var lobby:Object;
      
      public var layoutData:LayoutData;
      
      public var background:MovieClip;
      
      protected var _renderers:Array = new Array();
      
      protected var _height:int = 0;
      
      protected var _rendererTypes:Dictionary = new Dictionary();
      
      protected var _scrollContent:ScrollContent = new ScrollContent();
      
      protected var _rpRows:Dictionary;
      
      protected var _rpState:int;
      
      protected var _rpMatch:uint;

      // the player of the ask (REPLAY_WATCH_*)
      protected var _rpAsk:uint;

      // match id (8 hex) -> its chip's dictionary key (RP_CHIPS)
      protected var _rpChips:Object;
      
      protected var _rpPolls:int;
      
      protected var _rpTimer:Timer;
      
      protected var _rpPrepSeen:Boolean;
      
      protected var _rpAskAt:int;
      
      protected var _rpShownText:String;
      
      protected var _rpDialog:Object;
      
      protected var _rpDialogData:Object;
      
      protected var _rpShownKey:String;
      
      protected var _rpPopupWait:Boolean;
      
      public function NotificationListScrollContent()
      {
         super();
         _height = 0;
         this._scrollContent.width = 310;
         this._scrollContent.height = 469;
         this._scrollContent.x = 7;
         this._scrollContent.y = 7;
         this._scrollContent.fromBottom = true;
         this._scrollContent.autoScroll = true;
         this.historyInit();
         addChild(this._scrollContent);
      }
      
      public static function replayTake(rows:Array) : Boolean
      {
         var name:String = null;
         var taken:Boolean = false;
         try
         {
            if(rows && rows.length > 0 && rows[0])
            {
               name = String(rows[0].nickname);
               if(name.indexOf("R1;") == 0)
               {
                  taken = true;
                  if(NotificationListScrollContent.replayHolder)
                  {
                     NotificationListScrollContent.replayHolder.replayAnswer(name);
                  }
               }
            }
         }
         catch(e:Error)
         {
         }
         return taken;
      }
      
      override public function set x(value:Number) : void
      {
         super.x = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(value:Number) : void
      {
         super.y = int(value * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function get height() : Number
      {
         return 482;
      }
      
      override public function get width() : Number
      {
         return 420;
      }
      
      public function updateNotification(notification:Object) : void
      {
         var i:uint;
         while(i < this._renderers.length)
         {
            if(this._renderers[i].data == notification)
            {
               this._renderers[i].data = notification;
               this.historyUpdated(notification);
               return;
            }
            i++;
         }
      }
      
      public function registerRenderer(notificationType:int, rendererClass:Class) : void
      {
         this._rendererTypes[notificationType] = rendererClass;
      }
      
      public function addNotification(notification:Object, prepend:Boolean = false) : void
      {
         var notificationType:int = (notification.notificationType);
         var renderer:MovieClip;
         if(notificationType == NotificationType.MATCH)
         {
            renderer = new this._rendererTypes[notificationType]();
            renderer.addEventListener("showMessage",this.onMatchtMessage,false,0,true);
         }
         else if(notification.quest_label)
         {
            renderer = new this._rendererTypes[notificationType]();
            renderer.addEventListener("showMessage",this.onShowQuestMessage,false,0,true);
         }
         else
         {
            renderer = new this._rendererTypes[notificationType]();
            renderer.addEventListener("showMessage",this.onRelatedMessage,false,0,true);
         }
         this.historyWiden(renderer,notification);
         renderer.data = notification;
         renderer.validateNow();
         this.historyDecorate(renderer);
         renderer.y = this._height;
         _height += renderer.height + 1;
         this._scrollContent.addContent(renderer);
         this._scrollContent.validateNow();
         trace("NSLC::addNotification",renderer.y,this._scrollContent.scrolChildrens.y,this._scrollContent.y);
         this._renderers.push(renderer);
         this.background.y = Math.min(Math.max(this._scrollContent.scrolChildrens.y,0),this.height - 5);
         this.background.height = this.height - this.background.y;
      }
      
      private function onRelatedMessage(e:Event) : void
      {
         trace("onRelatedMessage::",e.target,e.currentTarget);
         if(e.target.data && e.target.data.type == ImportantMessageType.REPUTATION_LEVEL)
         {
            Message.showMessage("message_unlock_rank",e.target.data,false);
         }
      }
      
      private function onMatchtMessage(e:Event) : void
      {
         trace("onMatchtMessage::",e.target,e.currentTarget);
         if(e.target.data)
         {
            this.matchCalback(e.target.data);
         }
      }
      
      private function onShowQuestMessage(e:Event) : void
      {
         trace("onShowQuestMessage::",e.target,e.currentTarget);
         if(e.target.data && e.target.data.type == QUEST_MESSAGE_TYPE)
         {
            Message.showMessage("quest_message",e.target.data);
         }
      }
      
      public function reset() : void
      {
      }
      
      public function clear(force:Boolean = false) : void
      {
         var i:uint;
         while(i < this._renderers.length)
         {
            removeChild(this._renderers[i]);
            i++;
         }
         this._renderers.length = 0;
      }
      
      override public function toString() : String
      {
         return "[NotificationListScrollContent " + name + "]";
      }
      
      public function historyInit() : void
      {
         var i:int = 0;
         var child:DisplayObject = null;
         NotificationListScrollContent.replayHolder = this;
         this._rpRows = new Dictionary(true);
         this._rpChips = {};
         this._rpState = RP_IDLE;
         this._rpMatch = 0;
         this._rpAsk = NotificationListScrollContent.REPLAY_WATCH_ALL;
         this._rpPolls = 0;
         this._rpPrepSeen = false;
         this._rpAskAt = 0;
         this._rpShownText = "";
         this._rpShownKey = RP_KEY_NONE;
         this._scrollContent.width = 310 + NotificationListScrollContent.HISTORY_EXTRA;
         try
         {
            if(this.background)
            {
               this.background.width += NotificationListScrollContent.HISTORY_EXTRA;
            }
            i = 0;
            while(i < numChildren)
            {
               child = getChildAt(i);
               if(child && child != this.background && child is Shape && child.width > 310 && child.width < 330)
               {
                  child.width += NotificationListScrollContent.HISTORY_EXTRA;
               }
               i++;
            }
         }
         catch(e:Error)
         {
         }
      }
      
      public function historyUpdated(data:Object) : void
      {
         var i:int = 0;
         var row:MovieClip = null;
         try
         {
            i = 0;
            while(i < this._renderers.length)
            {
               row = this._renderers[i] as MovieClip;
               if(row && row.data == data)
               {
                  row.validateNow();
                  this.historyDecorate(row);
               }
               i++;
            }
         }
         catch(e:Error)
         {
         }
      }
      
      public function historyWiden(row:MovieClip, data:Object = null) : void
      {
         var rec:Object = {};
         var back:DisplayObject = null;
         var close:DisplayObject = null;
         var field:TextField = null;
         this.historyQuestIconFrame(data);
         try
         {
            back = row["background"] as DisplayObject;
            if(back)
            {
               rec.back = back.width + NotificationListScrollContent.HISTORY_EXTRA;
               back.width = rec.back;
            }
            close = row["btnClose"] as DisplayObject;
            if(close)
            {
               rec.close = close.x + NotificationListScrollContent.HISTORY_EXTRA;
               close.x = rec.close;
            }
            field = row["textField"] as TextField;
            if(field)
            {
               field.width += NotificationListScrollContent.HISTORY_EXTRA;
            }
            field = row["textFieldLabel"] as TextField;
            if(field)
            {
               field.width += NotificationListScrollContent.HISTORY_EXTRA;
            }
         }
         catch(e:Error)
         {
         }
         this._rpRows[row] = rec;
         row.addEventListener(ComponentEvent.STATE_CHANGE,this.onRpRowState,false,0,true);
         this.popupSchedule();
      }
      
      protected function popupSchedule() : void
      {
         if(this._rpPopupWait)
         {
            return;
         }
         this._rpPopupWait = true;
         addEventListener(Event.ENTER_FRAME,this.onPopupFrame,false,0,true);
      }
      
      protected function onPopupFrame(e:Event) : void
      {
         var lobby:Object = null;
         var list:DisplayObjectContainer = null;
         var i:int = 0;
         var row:MovieClip = null;
         var rec:Object = null;
         removeEventListener(Event.ENTER_FRAME,this.onPopupFrame);
         this._rpPopupWait = false;
         try
         {
            lobby = this.lobby;
            if(!lobby || !("notificationListTemp" in lobby))
            {
               return;
            }
            list = lobby["notificationListTemp"] as DisplayObjectContainer;
            if(!list || list == this)
            {
               return;
            }
            i = 0;
            while(i < list.numChildren)
            {
               row = list.getChildAt(i) as MovieClip;
               if(row && !this._rpRows[row] && "data" in row && row.data && ("textFieldResult" in row || "rewardField" in row || "questIcon" in row))
               {
                  rec = {"popup":true};
                  this._rpRows[row] = rec;
                  row.addEventListener(ComponentEvent.STATE_CHANGE,this.onRpRowState,false,0,true);
                  this.historyDecorate(row);
               }
               i++;
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function onRpRowState(e:Event) : void
      {
         var row:MovieClip = e.currentTarget as MovieClip;
         if(row)
         {
            this.historyKeep(row);
         }
      }
      
      public function historyQuestIconFrame(data:Object) : void
      {
         var frame:int = 1;
         try
         {
            if(data && int(data.notificationType) == NotificationType.QUEST && data.icon == undefined)
            {
               if(data.quest_icon != undefined)
               {
                  frame = int(data.quest_icon) + 1;
               }
               if(frame < 1 || frame > 15)
               {
                  frame = 1;
               }
               data.icon = frame;
            }
         }
         catch(e:Error)
         {
         }
      }
      
      public function historyRowParts(row:MovieClip, rec:Object) : void
      {
         this.historyRewardLayout(row,rec);
         this.historyQuestIcon(row,rec);
      }
      
      protected function historyRewardLayout(row:MovieClip, rec:Object) : void
      {
         var label:TextField = null;
         var bar:DisplayObject = null;
         var back:DisplayObject = null;
         var bounds:Rectangle = null;
         var right:Number = 286;
         var want:Number = 0;
         try
         {
            label = row["rewardField"] as TextField;
            bar = row["rewardBar"] as DisplayObject;
            if(!label || !bar || !label.visible || !bar.visible || label.text.length == 0)
            {
               return;
            }
            if(rec.barX0 == undefined)
            {
               rec.barX0 = bar.x;
            }
            back = row["background"] as DisplayObject;
            if(back)
            {
               right = back.width;
            }
            want = label.x + 2 + label.textWidth + 8;
            if(want < Number(rec.barX0))
            {
               want = Number(rec.barX0);
            }
            bounds = bar.getBounds(bar);
            if(want + bounds.x + bounds.width > right - 6)
            {
               want = right - 6 - bounds.x - bounds.width;
               if(want < Number(rec.barX0))
               {
                  want = Number(rec.barX0);
               }
            }
            if(bar.x != want)
            {
               bar.x = want;
            }
         }
         catch(e:Error)
         {
         }
      }
      
      protected function historyQuestIcon(row:MovieClip, rec:Object) : void
      {
         var icon:DisplayObject = null;
         var back:DisplayObject = null;
         var bounds:Rectangle = null;
         var high:Number = 0;
         var scale:Number = 0;
         var box:Number = 38;
         try
         {
            icon = row["questIcon"] as DisplayObject;
            back = row["background"] as DisplayObject;
            if(!icon || !back)
            {
               return;
            }
            bounds = icon.getBounds(icon);
            if(!(bounds.width > 0) || !(bounds.height > 0))
            {
               return;
            }
            high = back.height - 12;
            if(high < box)
            {
               box = high;
            }
            if(box < 16)
            {
               box = 16;
            }
            scale = box / Math.max(bounds.width,bounds.height);
            if(scale > 0.508)
            {
               scale = 0.508;
            }
            if(icon.scaleX != scale || icon.scaleY != scale)
            {
               icon.scaleX = scale;
               icon.scaleY = scale;
            }
            icon.x = 24 - scale * (bounds.x + bounds.width / 2);
            icon.y = (back.height - icon.height) / 2;
         }
         catch(e:Error)
         {
         }
      }
      
      protected function historyKeep(row:MovieClip) : void
      {
         var rec:Object = this._rpRows[row];
         var back:DisplayObject = null;
         var close:DisplayObject = null;
         if(!rec)
         {
            return;
         }
         try
         {
            back = row["background"] as DisplayObject;
            if(back && rec.back != undefined && back.width != rec.back)
            {
               back.width = rec.back;
            }
            close = row["btnClose"] as DisplayObject;
            if(close && rec.close != undefined && close.x != rec.close)
            {
               close.x = rec.close;
            }
            this.historyRowParts(row,rec);
            if(rec.ring)
            {
               this.historyPlaceRing(rec);
            }
            if(rec.result && rec.fitSize > 0)
            {
               if(rec.result.text != rec.fitText)
               {
                  this.historyFit(rec,rec.result.text);
               }
               else
               {
                  this.historyPlaceScore(rec);
               }
            }
         }
         catch(e:Error)
         {
         }
      }
      
      protected function historyRing(card:MovieClip) : DisplayObject
      {
         var i:int = 0;
         var child:DisplayObject = null;
         var ring:DisplayObject = null;
         try
         {
            i = card.numChildren - 1;
            while(i >= 0)
            {
               child = card.getChildAt(i);
               if(!ring && child && !(child is InteractiveObject))
               {
                  if(child.width > 23 && child.width < 30 && child.height > 23 && child.height < 30)
                  {
                     ring = child;
                  }
               }
               i--;
            }
         }
         catch(e:Error)
         {
            ring = null;
         }
         return ring;
      }
      
      protected function historyPlaceRing(rec:Object) : void
      {
         var ring:DisplayObject = rec.ring as DisplayObject;
         if(ring && !rec.drawn)
         {
            if(ring.scaleX != 1.4 || ring.scaleY != 1.4)
            {
               ring.scaleX = 1.4;
               ring.scaleY = 1.4;
            }
            ring.x = 27 - 1.4 * 29.775;
            ring.y = 31.5 - 1.4 * 32.525;
         }
         if(rec.time)
         {
            rec.time.x = 27 - 22.075 - 1.2;
            rec.time.y = 51;
         }
      }
      
      protected function historyPlaceScore(rec:Object) : void
      {
         var size:Number = Number(rec.fitSize);
         rec.result.x = 27 - 22.075 - 1.2 * size / 12;
         rec.result.y = 31.5 - 0.636 * size - 1.5 * size / 12;
      }
      
      protected function historyFit(rec:Object, text:String) : void
      {
         var field:TextField = rec.result as TextField;
         var format:TextFormat = rec.fmt as TextFormat;
         var size:int = 12;
         var room:Number = 14.9;
         var halfW:Number = 0;
         var halfH:Number = 0;
         if(!field || !format)
         {
            return;
         }
         if(text == null)
         {
            text = "";
         }
         field.wordWrap = false;
         while(true)
         {
            format.size = size;
            field.defaultTextFormat = format;
            field.text = text;
            field.setTextFormat(format);
            halfW = field.textWidth / 2;
            halfH = 0.7 * size / 2;
            if(text.length == 0 || size <= 6)
            {
               break;
            }
            if(halfW * halfW + halfH * halfH <= room * room && field.textHeight <= 2 * room)
            {
               break;
            }
            size--;
         }
         rec.fitText = field.text;
         rec.fitSize = size;
         this.historyPlaceScore(rec);
      }
      
      public function historyDecorate(row:MovieClip) : void
      {
         var rec:Object = this._rpRows[row];
         var data:Object = null;
         var result:TextField = null;
         var time:TextField = null;
         var cover:Shape = null;
         var step:int = 0;
         var angle:Number = 0;
         if(!rec)
         {
            return;
         }
         try
         {
            data = row.data;
            this.historyRowParts(row,rec);
            if(!data || int(data.notificationType) != NotificationType.MATCH)
            {
               return;
            }
            result = row["textFieldResult"] as TextField;
            time = row["textFieldTime"] as TextField;
            if(!result)
            {
               return;
            }
            if(!rec.decorated)
            {
               rec.decorated = true;
               rec.result = result;
               rec.time = time;
               if(time)
               {
                  time.wordWrap = false;
               }
               rec.fmt = result.text.length > 0 ? result.getTextFormat() : result.defaultTextFormat;
               rec.ring = this.historyRing(row);
               if(!rec.ring)
               {
                  cover = new Shape();
                  cover.graphics.beginFill(197636,1);
                  cover.graphics.moveTo(29.775 + 14.2,32.525);
                  step = 1;
                  while(step <= 48)
                  {
                     angle = step * Math.PI / 24;
                     cover.graphics.lineTo(29.775 + 14.2 * Math.cos(angle),32.525 + 14.2 * Math.sin(angle));
                     step++;
                  }
                  cover.graphics.endFill();
                  cover.graphics.lineStyle(1.4,10066329,0.5);
                  cover.graphics.moveTo(27 + 17.6,31.5);
                  step = 1;
                  while(step <= 48)
                  {
                     angle = step * Math.PI / 24;
                     cover.graphics.lineTo(27 + 17.6 * Math.cos(angle),31.5 + 17.6 * Math.sin(angle));
                     step++;
                  }
                  row.addChildAt(cover,row.getChildIndex(result));
                  rec.ring = cover;
                  rec.drawn = true;
               }
            }
            this.historyPlaceRing(rec);
            this.historyFit(rec,String(data.result != undefined ? data.result : result.text));
            if(!rec.watch && rec.popup != true && uint(data.match_id) != 0)
            {
               this.historyAddWatch(row,rec);
            }
            if(rec.watch)
            {
               this.historyChip(row,rec,this._rpChips[this.rpHex(uint(data.match_id))] as String);
               this.historyNumber(row,rec,uint(data.match_id));
            }
         }
         catch(e:Error)
         {
         }
      }
      
      // A match row's Watch (in this game), at its top right.
      protected function historyAddWatch(row:MovieClip, rec:Object) : void
      {
         var right:Number = 286 + NotificationListScrollContent.HISTORY_EXTRA;
         var map:TextField = null;
         if(rec.back != undefined)
         {
            right = Number(rec.back);
         }
         rec.watch = this.historyButton(row,"srpReplayWatch",this.rpText(RP_KEY_WATCH),right - 6,this.onRpWatchClick);
         map = row["textFieldLabel"] as TextField;
         if(map && map.x + map.width > rec.watch.x - 4)
         {
            map.width = rec.watch.x - 4 - map.x;
         }
      }

      // A small button ending at `right`, as wide as its label wants (70 to 120).
      protected function historyButton(row:Sprite, name:String, label:String, right:Number, click:Function) : Button
      {
         var buttonClass:Class = getDefinitionByName("SvButton2SmallFr") as Class;
         var button:Button = new buttonClass() as Button;
         var width:int = Math.max(70,Math.min(120,int(label.length * 7.3 + 18)));
         button.name = name;
         button.label = label;
         button.x = int(right - width);
         button.y = 5;
         button.addEventListener(MouseEvent.MOUSE_DOWN,this.onRpStopEvent,false,0,true);
         button.addEventListener(MouseEvent.CLICK,this.onRpStopEvent,false,0,true);
         button.addEventListener(MouseEvent.DOUBLE_CLICK,this.onRpStopEvent,false,0,true);
         button.addEventListener(ButtonEvent.PRESS,this.onRpStopEvent,false,0,true);
         button.addEventListener(ButtonEvent.CLICK,click,false,0,true);
         row.addChild(button);
         button.width = width;
         return button;
      }

      // A typed match number watched (the Watch window's box), or the prompt says what a number is.
      public function replayTyped(text:String) : void
      {
         var id:uint = NotificationListScrollContent.matchNumber(text);
         if(id == 0)
         {
            if(this._rpState != RP_ASKING)
            {
               this._rpState = RP_ENDED;
               this.rpShow(RP_KEY_NUMBER_BAD);
            }
            return;
         }
         this.replayBegin(id,NotificationListScrollContent.REPLAY_WATCH_ALL);
      }

      // A typed match number: 1 to 8 hex digits, an optional `#` and spaces around; 0 if not one.
      public static function matchNumber(text:String) : uint
      {
         var digits:String = text == null ? "" : text.replace(/^\s+|\s+$/g,"").replace(/^#/,"");
         if(!/^[0-9a-fA-F]{1,8}$/.test(digits))
         {
            return 0;
         }
         return uint(parseInt(digits,16));
      }

      // A row's number, `#` and 8 hex digits, as the box takes it.
      public static function matchNumberText(id:uint) : String
      {
         var text:String = id.toString(16).toLowerCase();
         while(text.length < 8)
         {
            text = "0" + text;
         }
         return "#" + text;
      }

      // The row's number, in the chip's line, right-aligned left of the chip (or of Watch's end).
      protected function historyNumber(row:MovieClip, rec:Object, id:uint) : void
      {
         var field:TextField = rec.number as TextField;
         var right:Number = Number(rec.watch.x) + Number(rec.watch.width);
         if(!field)
         {
            field = new TextField();
            field.name = "srpReplayNumberText";
            field.embedFonts = true;
            field.selectable = false;
            field.mouseEnabled = false;
            field.autoSize = TextFieldAutoSize.LEFT;
            field.wordWrap = false;
            field.defaultTextFormat = new TextFormat(Button(rec.watch).textField.getTextFormat().font,11,RP_NUMBER_GREY,true);
            field.text = NotificationListScrollContent.matchNumberText(id);
            row.addChild(field);
            rec.number = field;
         }
         if(rec.chip)
         {
            right = Number(rec.chip.x) - 6;
         }
         field.x = int(right - field.width);
         field.y = int(Number(rec.watch.y) + Number(rec.watch.height) + 4);
      }

      // The row's chip (`key` a dictionary key of RP_CHIPS, null none), under its buttons, right-aligned.
      protected function historyChip(row:MovieClip, rec:Object, key:String) : void
      {
         var chip:Sprite = rec.chip as Sprite;
         var field:TextField = null;
         var colour:uint = RP_CHIP_GREY;
         if(rec.chipKey === key)
         {
            return;
         }
         rec.chipKey = key;
         if(chip)
         {
            row.removeChild(chip);
            rec.chip = null;
         }
         if(key == null)
         {
            return;
         }
         if(RP_CHIP_COLOURS[key] != undefined)
         {
            colour = uint(RP_CHIP_COLOURS[key]);
         }
         field = new TextField();
         field.embedFonts = true;
         field.selectable = false;
         field.mouseEnabled = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.wordWrap = false;
         // the font of the Watch button's label
         field.defaultTextFormat = new TextFormat(Button(rec.watch).textField.getTextFormat().font,11,colour,true);
         field.text = this.rpText(key);
         field.x = 5;
         field.y = 0;
         chip = new Sprite();
         chip.name = "srpReplayChip";
         chip.mouseEnabled = false;
         chip.mouseChildren = false;
         chip.graphics.lineStyle(1,colour,0.9);
         chip.graphics.beginFill(colour,0.15);
         chip.graphics.drawRoundRect(0,0,int(field.width + 10),int(field.height),8,8);
         chip.graphics.endFill();
         chip.addChild(field);
         chip.x = int(Number(rec.watch.x) + Number(rec.watch.width) - chip.width);
         chip.y = int(Number(rec.watch.y) + Number(rec.watch.height) + 4);
         row.addChild(chip);
         rec.chip = chip;
      }

      // The chip's dictionary key the last answer about match `hex` set (RP_CHIPS), null none.
      public function chipOf(hex:String) : String
      {
         return this._rpChips[hex] as String;
      }

      // A chip's colour by its dictionary key: ready green, preparing amber, the others grey.
      public static function chipColour(key:String) : uint
      {
         return RP_CHIP_COLOURS[key] != undefined ? uint(RP_CHIP_COLOURS[key]) : RP_CHIP_GREY;
      }

      // The answer's word for the match's chip; the rows of that match drawn again.
      protected function rpChip(hex:String, status:String) : void
      {
         var key:String = RP_CHIPS[status] as String;
         var i:int = 0;
         var row:MovieClip = null;
         if(key == null)
         {
            return;
         }
         this._rpChips[hex] = key;
         if(NotificationListScrollContent.chipListener != null)
         {
            NotificationListScrollContent.chipListener(hex,status);
         }
         i = 0;
         while(i < this._renderers.length)
         {
            row = this._renderers[i] as MovieClip;
            if(row && row.data && this.rpHex(uint(row.data.match_id)) == hex)
            {
               this.historyDecorate(row);
            }
            i++;
         }
      }

      protected function onRpStopEvent(e:Event) : void
      {
         e.stopPropagation();
      }
      
      // The match of the row whose button was pressed (0: none).
      protected function rpRowMatch(e:Event) : uint
      {
         var row:MovieClip = null;
         var data:Object = null;
         var id:uint = 0;
         try
         {
            row = (e.currentTarget as DisplayObject).parent as MovieClip;
            if(row)
            {
               data = row.data;
            }
            if(data)
            {
               id = uint(data.match_id);
            }
         }
         catch(err:Error)
         {
         }
         return id;
      }

      protected function onRpWatchClick(e:Event) : void
      {
         var id:uint = this.rpRowMatch(e);
         e.stopPropagation();
         if(id != 0)
         {
            this.replayBegin(id,NotificationListScrollContent.REPLAY_WATCH_ALL);
         }
      }

      protected function rpText(key:String) : String
      {
         return LocalizationData.getName(key);
      }
      
      protected function rpFinalKey(status:String) : String
      {
         var key:String = RP_FINALS[status] as String;
         return key != null ? key : RP_KEY_NONE;
      }
      
      protected function rpHex(value:uint) : String
      {
         var text:String = value.toString(16).toLowerCase();
         while(text.length < 8)
         {
            text = "0" + text;
         }
         return text;
      }
      
      // Ask to watch match `id`'s replay in this game: `ask` a REPLAY_WATCH_* player. The History's
      // rows, the results screen and the Watch window ask through here.
      public function replayBegin(id:uint, ask:uint) : void
      {
         if(this._rpState == RP_ASKING)
         {
            return;
         }
         NotificationListScrollContent.replayHolder = this;
         this._rpMatch = id;
         this._rpAsk = ask;
         this._rpPolls = 0;
         this._rpPrepSeen = false;
         this._rpAskAt = getTimer();
         this._rpState = RP_ASKING;
         this.rpShow(RP_KEY_ASKING);
         try
         {
            this.rpCallAsk(this._rpAsk,this._rpMatch);
         }
         catch(e:Error)
         {
         }
         if(!this._rpTimer)
         {
            this._rpTimer = new Timer(1000,0);
            this._rpTimer.addEventListener(TimerEvent.TIMER,this.onRpTimer,false,0,true);
         }
         this._rpTimer.reset();
         this._rpTimer.start();
      }
      
      protected function rpCallAsk(player:uint, match:uint) : void
      {
         ExternalInterface.call("victory_screen_request_player_data",player,match);
      }
      
      protected function rpCallPoll(start:uint, count:int) : void
      {
         ExternalInterface.call("request_players_range",start,count);
      }
      
      protected function rpStop() : void
      {
         if(this._rpTimer)
         {
            this._rpTimer.stop();
         }
      }
      
      protected function onRpTimer(e:TimerEvent) : void
      {
         var waited:int = 0;
         if(this._rpState != RP_ASKING)
         {
            this.rpStop();
            return;
         }
         this._rpPolls += 1;
         if(this._rpPrepSeen)
         {
            waited = getTimer() - this._rpAskAt;
            if(waited >= NotificationListScrollContent.REPLAY_PREPARE_MS)
            {
               this.rpStop();
               this._rpState = RP_ENDED;
               this.rpShow(RP_KEY_STILL_PREPARING);
               return;
            }
            this.rpShowPreparing();
         }
         else if(this._rpPolls > NotificationListScrollContent.REPLAY_POLLS)
         {
            this.rpStop();
            this._rpState = RP_ENDED;
            this.rpShow(RP_KEY_NO_ANSWER);
            return;
         }
         try
         {
            this.rpCallPoll(NotificationListScrollContent.REPLAY_START,1);
         }
         catch(err:Error)
         {
         }
      }
      
      public function replayAnswer(name:String) : void
      {
         var fields:Array = name.split(";");
         var status:String = fields.length > 1 ? String(fields[1]) : "";
         var hex:String = fields.length > 2 ? String(fields[2]).toLowerCase() : "";
         var key:String = RP_KEY_NONE;
         if(this._rpState != RP_ASKING)
         {
            return;
         }
         if(hex != this.rpHex(this._rpMatch))
         {
            return;
         }
         this.rpChip(hex,status);
         if(status == "watching")
         {
            this.rpStop();
            this._rpState = RP_ENDED;
            this.rpShow(RP_KEY_WATCHING);
            return;
         }
         if(status == "preparing")
         {
            this._rpPrepSeen = true;
            this.rpShowPreparing();
            return;
         }
         key = this.rpFinalKey(status);
         if(key != null)
         {
            this.rpStop();
            this._rpState = RP_ENDED;
            this.rpShow(key);
            return;
         }
      }
      
      protected function rpPreparingText() : String
      {
         var seconds:int = int((getTimer() - this._rpAskAt) / 1000);
         if(seconds < 0)
         {
            seconds = 0;
         }
         return LocalizationData.format(RP_KEY_OPENING,LocalizationData.format("st_srp_replay_waiting",seconds));
      }
      
      protected function rpShowPreparing() : void
      {
         var text:String = this.rpPreparingText();
         var message:Object = this._rpDialog;
         var high:Number = 0;
         if(this._rpShownKey != RP_KEY_OPENING || !message || !message.textField)
         {
            this.rpShow(RP_KEY_OPENING);
            return;
         }
         if(text == this._rpShownText)
         {
            return;
         }
         try
         {
            high = message.textField.textHeight;
            message.textField.text = text;
            this._rpShownText = text;
            if(message.textField.textHeight != high)
            {
               this.rpShow(RP_KEY_OPENING);
            }
         }
         catch(e:Error)
         {
            this.rpShow(RP_KEY_OPENING);
         }
      }
      
      // The prompt with `key`'s text and Close alone.
      protected function rpShow(key:String) : void
      {
         var text:String = this.rpText(key);
         var data:Object = null;
         var message:Object = null;
         var format:TextFormat = null;
         if(key == RP_KEY_OPENING)
         {
            text = this.rpPreparingText();
         }
         this.rpDropDialog();
         this._rpShownText = text;
         data = {
            "text":text,
            "label_ok":null,
            "label_cancel":this.rpText(RP_KEY_CLOSE),
            "onYes":this.onRpNo,
            "onNo":this.onRpNo
         };
         this._rpDialogData = data;
         this._rpShownKey = key;
         try
         {
            message = Message.showMessage("apply",data,true);
            this._rpDialog = message;
            if(message)
            {
               if(text.length > 120 && message.textField)
               {
                  format = message.textField.defaultTextFormat;
                  format.size = 16;
                  message.textField.defaultTextFormat = format;
               }
               message.validateNow();
               if(message.btnAccept && message.btnCancel && message.background)
               {
                  message.btnAccept.visible = false;
                  message.btnAccept.enabled = false;
                  message.btnCancel.x = int(message.background.x + (message.background.width - message.btnCancel.width) / 2);
               }
            }
         }
         catch(e:Error)
         {
         }
      }
      
      protected function rpDropDialog() : void
      {
         var message:Object = this._rpDialog;
         if(this._rpDialogData)
         {
            this._rpDialogData.onYes = null;
            this._rpDialogData.onNo = null;
         }
         this._rpDialog = null;
         this._rpDialogData = null;
         this._rpShownKey = RP_KEY_NONE;
         if(message)
         {
            try
            {
               Message.close(uint(message.getMessageId()),true);
            }
            catch(e:Error)
            {
            }
         }
      }
      
      protected function onRpNo() : void
      {
         this.rpStop();
         this._rpState = RP_IDLE;
         this._rpDialog = null;
         this._rpDialogData = null;
         this._rpShownKey = RP_KEY_NONE;
      }
   }
}

