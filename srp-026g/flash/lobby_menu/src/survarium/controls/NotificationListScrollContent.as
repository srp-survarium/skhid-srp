package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.InteractiveObject;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.events.TimerEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Rectangle;
   import flash.text.TextField;
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
   import survarium.constants.Faction;
   import survarium.constants.ImportantMessageType;
   import survarium.constants.Language;
   import survarium.constants.MoneyType;
   import survarium.constants.NotificationType;
   import survarium.constants.QuestStatus;
   import survarium.constants.Reward;
   
   public class NotificationListScrollContent extends MovieClip
   {
      
      public static const REPLAY_DEMO:Boolean = false;
      
      public static const HISTORY_EXTRA:int = 100;
      
      public static const REPLAY_ASK:uint = 4294967041;
      
      public static const REPLAY_START:uint = 2200000000;
      
      public static const REPLAY_POLLS:int = 30;
      
      public static const REPLAY_PREPARE_MS:int = 180000;
      
      public static const REPLAY_DEMO_PREPARE_MS:int = 30000;
      
      // srp's replay prompt: its state (_rpState).
      private static const RP_IDLE:int = 0;
      
      // asking the match server, polling
      private static const RP_ASKING:int = 1;
      
      // the replay is ready to download
      private static const RP_READY:int = 2;
      
      // a final message is shown: nothing to fetch
      private static const RP_ENDED:int = 3;
      
      // the download was started
      private static const RP_STARTED:int = 4;
      
      // The prompt's texts: an index into rpWords (rpText, rpFill, rpShow); RP_KEY_NONE is no text shown.
      private static const RP_KEY_NONE:int = -1;
      
      private static const RP_KEY_TITLE:int = 0;
      
      private static const RP_KEY_DOWNLOAD:int = 1;
      
      private static const RP_KEY_CLOSE:int = 2;
      
      private static const RP_KEY_ASKING:int = 3;
      
      private static const RP_KEY_BEING_MADE:int = 4;
      
      private static const RP_KEY_READY_SIZE:int = 5;
      
      private static const RP_KEY_READY:int = 6;
      
      // the first of the final answers (rpFinalKey): not_recorded, gone, not_yours, old_game, incompatible, busy, failed, off
      private static const RP_KEY_FIRST_FINAL:int = 7;
      
      private static const RP_KEY_NO_ANSWER:int = 15;
      
      private static const RP_KEY_STARTING:int = 16;
      
      private static const RP_KEY_MB:int = 17;
      
      private static const RP_KEY_PLACE_IN_LINE:int = 18;
      
      private static const RP_KEY_MADE_NOW:int = 19;
      
      private static const RP_KEY_WAITING:int = 20;
      
      private static const RP_KEY_STILL_PREPARING:int = 21;
      
      // The highest place in the replay server's line the prompt shows.
      private static const RP_PLACE_MAX:int = 999;
      
      // meaning unknown: onShowQuestMessage shows the quest message only for a notification of this type
      private static const QUEST_MESSAGE_TYPE:int = 3;
      
      public static var replayHolder:NotificationListScrollContent;
      
      public var matchCalback:Function;
      
      public var layoutData:LayoutData;
      
      public var background:MovieClip;
      
      protected var _renderers:Array = new Array();
      
      protected var _height:int = 0;
      
      protected var _rendererTypes:Dictionary = new Dictionary();
      
      protected var _scrollContent:ScrollContent = new ScrollContent();
      
      protected var _rpRows:Dictionary;
      
      protected var _rpState:int;
      
      protected var _rpMatch:uint;
      
      protected var _rpPolls:int;
      
      protected var _rpTimer:Timer;
      
      protected var _rpStatus:String;
      
      protected var _rpHex:String;
      
      protected var _rpSize:Number;
      
      protected var _rpPlace:int;
      
      protected var _rpPrepSeen:Boolean;
      
      protected var _rpAskAt:int;
      
      protected var _rpShownText:String;
      
      protected var _rpDialog:Object;
      
      protected var _rpDialogData:Object;
      
      protected var _rpShownKey:int;
      
      protected var _rpDemoStep:int;
      
      protected var _rpDemoAdded:Boolean;
      
      protected var _rpPopupWait:Boolean;
      
      protected var _rpDemoTempCallback:Function;
      
      protected var _rpDemoSeenAt:int;
      
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
         this.historyRegistered(notificationType);
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
         if(this.historyDemoEvent(e))
         {
            return;
         }
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
         this._rpRows = new Dictionary(true);
         this._rpState = RP_IDLE;
         this._rpMatch = 0;
         this._rpPolls = 0;
         this._rpStatus = "";
         this._rpHex = "";
         this._rpSize = 0;
         this._rpPlace = 0;
         this._rpPrepSeen = false;
         this._rpAskAt = 0;
         this._rpShownText = "";
         this._rpShownKey = RP_KEY_NONE;
         this._rpDemoStep = 0;
         this._rpDemoAdded = false;
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
      
      public function historyRegistered(type:int) : void
      {
         if(NotificationListScrollContent.REPLAY_DEMO && type == NotificationType.MATCH)
         {
            addEventListener(Event.ENTER_FRAME,this.onRpDemoFrame,false,0,true);
         }
      }
      
      public function historyDemoEvent(e:Event) : Boolean
      {
         var data:Object = null;
         try
         {
            data = e.target.data;
            return data != null && data.srpDemo == true;
         }
         catch(err:Error)
         {
         }
         return false;
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
            lobby = parent;
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
            if(!rec.button && rec.popup != true && (uint(data.match_id) != 0 || data.srpDemo == true || NotificationListScrollContent.REPLAY_DEMO))
            {
               this.historyAddButton(row,rec);
            }
         }
         catch(e:Error)
         {
         }
      }
      
      protected function historyAddButton(row:MovieClip, rec:Object) : void
      {
         var buttonClass:Class = getDefinitionByName("SvButton2SmallFr") as Class;
         var button:Button = new buttonClass() as Button;
         var label:String = this.rpText(RP_KEY_TITLE);
         var width:int = int(label.length * 7.3 + 18);
         var right:Number = 286 + NotificationListScrollContent.HISTORY_EXTRA;
         var map:TextField = null;
         if(width < 100)
         {
            width = 100;
         }
         if(width > 160)
         {
            width = 160;
         }
         if(rec.back != undefined)
         {
            right = Number(rec.back);
         }
         button.name = "srpReplay";
         button.label = label;
         button.x = int(right - 6 - width);
         button.y = 5;
         button.addEventListener(MouseEvent.MOUSE_DOWN,this.onRpStopEvent,false,0,true);
         button.addEventListener(MouseEvent.CLICK,this.onRpStopEvent,false,0,true);
         button.addEventListener(MouseEvent.DOUBLE_CLICK,this.onRpStopEvent,false,0,true);
         button.addEventListener(ButtonEvent.PRESS,this.onRpStopEvent,false,0,true);
         button.addEventListener(ButtonEvent.CLICK,this.onRpButtonClick,false,0,true);
         row.addChild(button);
         button.width = width;
         rec.button = button;
         map = row["textFieldLabel"] as TextField;
         if(map && map.x + map.width > button.x - 4)
         {
            map.width = button.x - 4 - map.x;
         }
      }
      
      protected function onRpStopEvent(e:Event) : void
      {
         e.stopPropagation();
      }
      
      protected function onRpButtonClick(e:Event) : void
      {
         var row:MovieClip = null;
         var data:Object = null;
         var id:uint = 0;
         e.stopPropagation();
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
            if(id == 0 && (NotificationListScrollContent.REPLAY_DEMO || data && data.srpDemo == true))
            {
               id = 484727998;
            }
            if(id != 0)
            {
               this.replayBegin(id);
            }
         }
         catch(err:Error)
         {
         }
      }
      
      protected function rpLang() : int
      {
         var word:String = LocalizationData.getName("btn_friends");
         if(word == "Друзі")
         {
            return Language.UK;
         }
         if(word == "Друзья")
         {
            return Language.RU;
         }
         if(word == "Prijatelji")
         {
            return Language.SR;
         }
         return Language.EN;
      }
      
      protected function rpWords() : Array
      {
         var words:Array = [
            ["Watch replay", "Дивитися повтор", "Смотреть повтор", "Watch replay", "Watch replay", "Gledaj snimak"],
            ["DOWNLOAD AND WATCH", "ЗАВАНТАЖИТИ Й ДИВИТИСЯ", "СКАЧАТЬ И СМОТРЕТЬ", "DOWNLOAD AND WATCH", "DOWNLOAD AND WATCH", "PREUZMI I GLEDAJ"],
            ["CLOSE", "ЗАКРИТИ", "ЗАКРЫТЬ", "CLOSE", "CLOSE", "ZATVORI"],
            ["Asking the match server for the replay of this match…", "Запитуємо в сервера матчу повтор цього матчу…", "Запрашиваем у сервера матча повтор этого матча…", "Asking the match server for the replay of this match…", "Asking the match server for the replay of this match…", "Traženje snimka ovog meča od servera…"],
            [["The replay is being made from the match record. This happens the first time anyone watches this match and takes a little while.\nYou can close this window: the server keeps making it.\n\n", ""], ["Повтор створюється із запису матчу. Так буває, коли цей матч дивляться вперше, і це займає трохи часу.\nЦе вікно можна закрити: сервер продовжить його створювати.\n\n", ""], ["Повтор создаётся из записи матча. Так бывает, когда этот матч смотрят впервые, и это занимает немного времени.\nЭто окно можно закрыть: сервер продолжит его создавать.\n\n", ""], ["The replay is being made from the match record. This happens the first time anyone watches this match and takes a little while.\nYou can close this window: the server keeps making it.\n\n", ""], ["The replay is being made from the match record. This happens the first time anyone watches this match and takes a little while.\nYou can close this window: the server keeps making it.\n\n", ""], ["Snimak se pravi iz zapisa meča. To se dešava kada neko prvi put gleda ovaj meč i može da potraje.\nMožeš zatvoriti prozor: server će nastaviti izradu.\n\n", ""]],
            [["Replay of this match: ", ".\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed."], ["Повтор цього матчу: ", ".\n\nВін завантажується із сервера матчу, який зберігає достовірний запис матчу. Відкриється друге вікно гри й відтворить його; лобі залишиться відкритим.\n\nУ повторі натисніть F11 для керування: гравці, камера, перемотування, швидкість."], ["Повтор этого матча: ", ".\n\nОн загружается с сервера матча, который хранит достоверную запись матча. Откроется второе окно игры и воспроизведёт его; лобби останется открытым.\n\nВ повторе нажмите F11 для управления: игроки, камера, перемотка, скорость."], ["Replay of this match: ", ".\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed."], ["Replay of this match: ", ".\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed."], ["Snimak ovog meča: ", ".\n\nPreuzima se sa servera koji čuva izvorni zapis meča. Otvoriće se drugi prozor igre i prikazati snimak; lobi ostaje otvoren.\n\nU snimku pritisni F11 za kontrole: igrači, kamera, premotavanje, brzina."]],
            ["The replay of this match is ready.\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed.", "Повтор цього матчу готовий.\n\nВін завантажується із сервера матчу, який зберігає достовірний запис матчу. Відкриється друге вікно гри й відтворить його; лобі залишиться відкритим.\n\nУ повторі натисніть F11 для керування: гравці, камера, перемотування, швидкість.", "Повтор этого матча готов.\n\nОн загружается с сервера матча, который хранит достоверную запись матча. Откроется второе окно игры и воспроизведёт его; лобби останется открытым.\n\nВ повторе нажмите F11 для управления: игроки, камера, перемотка, скорость.", "The replay of this match is ready.\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed.", "The replay of this match is ready.\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed.", "Snimak ovog meča je spreman.\n\nPreuzima se sa servera koji čuva izvorni zapis meča. Otvoriće se drugi prozor igre i prikazati snimak; lobi ostaje otvoren.\n\nU snimku pritisni F11 za kontrole: igrači, kamera, premotavanje, brzina."],
            ["This match was not recorded: it had fewer than 6 players, or recording was off at the time.", "Цей матч не записано: у ньому було менше 6 гравців або запис тоді був вимкнений.", "Этот матч не записан: в нём было меньше 6 игроков или запись тогда была выключена.", "This match was not recorded: it had fewer than 6 players, or recording was off at the time.", "This match was not recorded: it had fewer than 6 players, or recording was off at the time.", "Ovaj meč nije snimljen: imao je manje od 6 igrača ili je snimanje tada bilo isključeno."],
            ["The replay of this match is no longer kept on the server.", "Повтор цього матчу більше не зберігається на сервері.", "Повтор этого матча больше не хранится на сервере.", "The replay of this match is no longer kept on the server.", "The replay of this match is no longer kept on the server.", "Snimak ovog meča više se ne čuva na serveru."],
            ["You can watch replays only of matches you played in.", "Дивитися можна лише повтори матчів, у яких ви грали.", "Смотреть можно только повторы матчей, в которых вы играли.", "You can watch replays only of matches you played in.", "You can watch replays only of matches you played in.", "Možeš gledati samo snimke mečeva u kojima si učestvovao."],
            ["Your game version cannot play replays. Update the game: start it again through the launcher.", "Ваша версія гри не відтворює повтори. Оновіть гру: запустіть її знову через лаунчер.", "Ваша версия игры не воспроизводит повторы. Обновите игру: запустите её снова через лаунчер.", "Your game version cannot play replays. Update the game: start it again through the launcher.", "Your game version cannot play replays. Update the game: start it again through the launcher.", "Tvoja verzija igre ne podržava snimke. Ažuriraj igru: ponovo je pokreni preko pokretača."],
            ["This replay was recorded by a game version that yours cannot play.", "Цей повтор записала версія гри, яку ваша гра не може відтворити.", "Этот повтор записан версией игры, которую ваша игра не может воспроизвести.", "This replay was recorded by a game version that yours cannot play.", "This replay was recorded by a game version that yours cannot play.", "Ovaj snimak je napravljen verzijom igre koju tvoja igra ne može da reprodukuje."],
            ["The server is busy with other replays right now. Try again in a minute.", "Сервер зараз зайнятий іншими повторами. Спробуйте ще раз за хвилину.", "Сервер сейчас занят другими повторами. Попробуйте ещё раз через минуту.", "The server is busy with other replays right now. Try again in a minute.", "The server is busy with other replays right now. Try again in a minute.", "Server je trenutno zauzet drugim snimcima. Pokušaj ponovo za minut."],
            ["The server could not prepare the replay of this match.", "Серверу не вдалося підготувати повтор цього матчу.", "Серверу не удалось подготовить повтор этого матча.", "The server could not prepare the replay of this match.", "The server could not prepare the replay of this match.", "Server nije uspeo da pripremi snimak ovog meča."],
            ["Replays are switched off on this server.", "Повтори на цьому сервері вимкнено.", "Повторы на этом сервере отключены.", "Replays are switched off on this server.", "Replays are switched off on this server.", "Snimci su isključeni na ovom serveru."],
            ["The server did not answer. Try again later.", "Сервер не відповів. Спробуйте пізніше.", "Сервер не ответил. Попробуйте позже.", "The server did not answer. Try again later.", "The server did not answer. Try again later.", "Server nije odgovorio. Pokušaj kasnije."],
            ["Starting the replay: a second game window will open; this lobby stays open. If something goes wrong, the updater shows what happened.\n\nIn the replay press F11 for the controls.", "Запускаємо повтор: відкриється друге вікно гри, лобі залишиться відкритим. Якщо щось піде не так, програма оновлення покаже, що сталося.\n\nУ повторі натисніть F11 для керування.", "Запускаем повтор: откроется второе окно игры, лобби останется открытым. Если что-то пойдёт не так, программа обновления покажет, что случилось.\n\nВ повторе нажмите F11 для управления.", "Starting the replay: a second game window will open; this lobby stays open. If something goes wrong, the updater shows what happened.\n\nIn the replay press F11 for the controls.", "Starting the replay: a second game window will open; this lobby stays open. If something goes wrong, the updater shows what happened.\n\nIn the replay press F11 for the controls.", "Pokretanje snimka: otvoriće se drugi prozor igre; ovaj lobi ostaje otvoren. Ako nešto pođe po zlu, program za ažuriranje će prikazati šta se desilo.\n\nU snimku pritisni F11 za kontrole."],
            ["MB", "МБ", "МБ", "MB", "MB", "MB"],
            [["Place in line: ", ". Waiting: ", " s."], ["Місце в черзі: ", ". Очікування: ", " с."], ["Место в очереди: ", ". Ожидание: ", " с."], ["Place in line: ", ". Waiting: ", " s."], ["Place in line: ", ". Waiting: ", " s."], ["Mesto u redu: ", ". Čekanje: ", " s."]],
            [["Being made now. Waiting: ", " s."], ["Створюється зараз. Очікування: ", " с."], ["Создаётся сейчас. Ожидание: ", " с."], ["Being made now. Waiting: ", " s."], ["Being made now. Waiting: ", " s."], ["Izrada je u toku. Čekanje: ", " s."]],
            [["Waiting: ", " s."], ["Очікування: ", " с."], ["Ожидание: ", " с."], ["Waiting: ", " s."], ["Waiting: ", " s."], ["Čekanje: ", " s."]],
            ["Still being prepared. It keeps being made on the server; open Watch replay again in a minute.", "Повтор ще готується. Сервер продовжує його створювати; відкрийте «Дивитися повтор» ще раз за хвилину.", "Повтор ещё готовится. Сервер продолжает его создавать; откройте «Смотреть повтор» ещё раз через минуту.", "Still being prepared. It keeps being made on the server; open Watch replay again in a minute.", "Still being prepared. It keeps being made on the server; open Watch replay again in a minute.", "Snimak se još priprema. Server nastavlja izradu; ponovo otvori Gledaj snimak za minut."]
         ];
         return words;
      }
      
      protected function rpText(key:int) : String
      {
         var words:Array = this.rpWords();
         var entry:Object = null;
         if(key < 0 || key >= words.length)
         {
            return "";
         }
         entry = words[key][this.rpLang()];
         if(entry is Array)
         {
            return this.rpFill(key,[]);
         }
         return String(entry);
      }
      
      protected function rpFill(key:int, values:Array) : String
      {
         var words:Array = this.rpWords();
         var pieces:Array = null;
         var text:String = "";
         var i:int = 0;
         if(key < 0 || key >= words.length)
         {
            return "";
         }
         pieces = words[key][this.rpLang()] as Array;
         if(!pieces)
         {
            return String(words[key][this.rpLang()]);
         }
         text = String(pieces[0]);
         i = 1;
         while(i < pieces.length)
         {
            if(values && i - 1 < values.length)
            {
               text += String(values[i - 1]);
            }
            text += String(pieces[i]);
            i++;
         }
         return text;
      }
      
      protected function rpFinalKey(status:String) : int
      {
         var finals:Array = ["not_recorded","gone","not_yours","old_game","incompatible","busy","failed","off"];
         var i:int = 0;
         i = 0;
         while(i < finals.length)
         {
            if(finals[i] == status)
            {
               return RP_KEY_FIRST_FINAL + i;
            }
            i++;
         }
         return RP_KEY_NONE;
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
      
      protected function rpSizeText(bytes:Number) : String
      {
         var tenths:Number = 0;
         var text:String = null;
         if(!(bytes > 0))
         {
            return "";
         }
         tenths = Math.round(bytes / 104857.6);
         if(tenths < 1)
         {
            tenths = 1;
         }
         text = String(Math.floor(tenths / 10)) + (this.rpLang() != Language.EN ? "," : ".") + String(tenths % 10);
         return text + " " + this.rpText(RP_KEY_MB);
      }
      
      public function replayBegin(id:uint) : void
      {
         if(this._rpState == RP_ASKING)
         {
            return;
         }
         NotificationListScrollContent.replayHolder = this;
         if(NotificationListScrollContent.REPLAY_DEMO)
         {
            this._rpDemoStep += 1;
         }
         this._rpMatch = id;
         this._rpPolls = 0;
         this._rpStatus = "";
         this._rpHex = "";
         this._rpSize = 0;
         this._rpPlace = 0;
         this._rpPrepSeen = false;
         this._rpAskAt = getTimer();
         this._rpState = RP_ASKING;
         this.rpShow(RP_KEY_ASKING,false);
         if(!NotificationListScrollContent.REPLAY_DEMO)
         {
            try
            {
               this.rpCallAsk(NotificationListScrollContent.REPLAY_ASK,this._rpMatch);
            }
            catch(e:Error)
            {
            }
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
            if(waited >= this.rpPrepareLimit())
            {
               this.rpStop();
               this._rpState = RP_ENDED;
               this.rpShow(RP_KEY_STILL_PREPARING,false);
               return;
            }
            this.rpShowPreparing();
         }
         else if(this._rpPolls > NotificationListScrollContent.REPLAY_POLLS)
         {
            this.rpStop();
            this._rpState = RP_ENDED;
            this.rpShow(RP_KEY_NO_ANSWER,false);
            return;
         }
         if(NotificationListScrollContent.REPLAY_DEMO)
         {
            this.rpDemoAnswer(this._rpPolls);
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
         var size:Number = fields.length > 3 ? Number(fields[3]) : 0;
         var key:int = RP_KEY_NONE;
         if(this._rpState != RP_ASKING)
         {
            return;
         }
         if(hex != this.rpHex(this._rpMatch))
         {
            return;
         }
         if(!(size >= 0))
         {
            size = 0;
         }
         this._rpStatus = status;
         this._rpHex = hex;
         if(status == "preparing")
         {
            this._rpPrepSeen = true;
            this._rpPlace = int(Math.min(size,RP_PLACE_MAX));
            this.rpShowPreparing();
            return;
         }
         this._rpSize = size;
         if(status == "ready")
         {
            this.rpStop();
            this._rpState = RP_READY;
            this.rpShow(this._rpSize > 0 ? RP_KEY_READY_SIZE : RP_KEY_READY,true);
            return;
         }
         key = this.rpFinalKey(status);
         if(key >= 0)
         {
            this.rpStop();
            this._rpState = RP_ENDED;
            this.rpShow(key,false);
            return;
         }
      }
      
      protected function rpPrepareLimit() : int
      {
         if(NotificationListScrollContent.REPLAY_DEMO)
         {
            return NotificationListScrollContent.REPLAY_DEMO_PREPARE_MS;
         }
         return NotificationListScrollContent.REPLAY_PREPARE_MS;
      }
      
      protected function rpPreparingText() : String
      {
         var seconds:int = int((getTimer() - this._rpAskAt) / 1000);
         var line:String = null;
         if(seconds < 0)
         {
            seconds = 0;
         }
         if(this._rpPlace > 1)
         {
            line = this.rpFill(RP_KEY_PLACE_IN_LINE,[String(this._rpPlace),String(seconds)]);
         }
         else if(this._rpPlace == 1)
         {
            line = this.rpFill(RP_KEY_MADE_NOW,[String(seconds)]);
         }
         else
         {
            line = this.rpFill(RP_KEY_WAITING,[String(seconds)]);
         }
         return this.rpFill(RP_KEY_BEING_MADE,[line]);
      }
      
      protected function rpShowPreparing() : void
      {
         var text:String = this.rpPreparingText();
         var message:Object = this._rpDialog;
         var high:Number = 0;
         if(this._rpShownKey != RP_KEY_BEING_MADE || !message || !message.textField)
         {
            this.rpShow(RP_KEY_BEING_MADE,false);
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
               this.rpShow(RP_KEY_BEING_MADE,false);
            }
         }
         catch(e:Error)
         {
            this.rpShow(RP_KEY_BEING_MADE,false);
         }
      }
      
      protected function rpShow(key:int, download:Boolean) : void
      {
         var text:String = this.rpText(key);
         var data:Object = null;
         var message:Object = null;
         var format:TextFormat = null;
         if(key == RP_KEY_READY_SIZE)
         {
            text = this.rpFill(RP_KEY_READY_SIZE,[this.rpSizeText(this._rpSize)]);
         }
         if(key == RP_KEY_BEING_MADE)
         {
            text = this.rpPreparingText();
         }
         this.rpDropDialog();
         this._rpShownText = text;
         data = {
            "text":text,
            "label_ok":(download ? this.rpText(RP_KEY_DOWNLOAD) : null),
            "label_cancel":this.rpText(RP_KEY_CLOSE),
            "onYes":this.onRpYes,
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
               if(!download && message.btnAccept && message.btnCancel && message.background)
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
      
      protected function onRpYes(data:Object = null) : void
      {
         var ready:Boolean = this._rpState == RP_READY && this._rpStatus == "ready" && this._rpHex == this.rpHex(this._rpMatch);
         this.rpStop();
         this._rpDialog = null;
         this._rpDialogData = null;
         this._rpShownKey = RP_KEY_NONE;
         if(!ready)
         {
            this._rpState = RP_IDLE;
            return;
         }
         this._rpState = RP_STARTED;
         if(!NotificationListScrollContent.REPLAY_DEMO)
         {
            try
            {
               ExternalInterface.call("video_tutorial_link_clicked");
            }
            catch(e:Error)
            {
            }
         }
         this.rpShow(RP_KEY_STARTING,false);
      }
      
      protected function rpDemoAnswer(poll:int) : void
      {
         var cases:Array = [["none","preparing:0","preparing:3","preparing:3","preparing:3","preparing:2","preparing:2","preparing:2","preparing:1","preparing:1","preparing:1","preparing:1","ready"],["preparing:2"],["not_recorded"],["gone"],["not_yours"],["old_game"],["incompatible"],["busy"],["failed"],["off"],["other","ready0"],["none"]];
         var which:int = (this._rpDemoStep - 1) % cases.length;
         var answers:Array = null;
         var word:String = null;
         var hex:String = this.rpHex(this._rpMatch);
         var size:String = "4404019";
         if(which < 0)
         {
            which = 0;
         }
         answers = cases[which];
         word = answers[Math.min(poll,answers.length) - 1];
         if(word.indexOf("preparing:") == 0)
         {
            size = word.substr(10);
            word = "preparing";
         }
         if(word == "other")
         {
            hex = this.rpHex(uint(this._rpMatch + 1));
            word = "ready";
         }
         if(word == "ready0")
         {
            word = "ready";
            size = "0";
         }
         NotificationListScrollContent.replayTake([{
            "nickname":"R1;" + word + ";" + hex + ";" + size,
            "place":0,
            "ranking":0,
            "ranking_diff":0
         }]);
      }
      
      protected function onRpDemoFrame(e:Event) : void
      {
         var lobby:Object = null;
         var now:int = getTimer();
         if(this._rpDemoAdded)
         {
            removeEventListener(Event.ENTER_FRAME,this.onRpDemoFrame);
            return;
         }
         try
         {
            lobby = parent;
            if(!stage || !lobby || !lobby["bottom_panel"] || !(lobby["add_notification"] is Function))
            {
               return;
            }
         }
         catch(err:Error)
         {
            return;
         }
         if(this._rpDemoSeenAt == 0)
         {
            this._rpDemoSeenAt = now;
            if(this._rpDemoSeenAt == 0)
            {
               this._rpDemoSeenAt = 1;
            }
            return;
         }
         if(now - this._rpDemoSeenAt < 3000)
         {
            return;
         }
         this._rpDemoSeenAt = 0;
         this._rpDemoAdded = true;
         removeEventListener(Event.ENTER_FRAME,this.onRpDemoFrame);
         this.rpDemoAddRows(lobby);
      }
      
      public function rpDemoTempMatch(data:Object) : void
      {
         if(data && data.srpDemo == true)
         {
            return;
         }
         if(this._rpDemoTempCallback != null)
         {
            this._rpDemoTempCallback(data);
         }
      }
      
      protected function rpDemoOtherRows() : Array
      {
         var lang:int = this.rpLang();
         var quest:String = ["Victory contribution","Внесок у перемогу","Вклад в победу", "Victory contribution", "Victory contribution", "Doprinos pobedi"][lang];
         var seller:String = GangShop.getSellerName(Faction.SCAVENGERS);
         var rankText:Array = [": rank 3 reached. New items are available in the shop.",": досягнуто ранг 3. У крамниці з\'явилися нові предмети.",": достигнут ранг 3. В магазине появились новые предметы.", ": rank 3 reached. New items are available in the shop.", ": rank 3 reached. New items are available in the shop.", ": dostignut rang 3. Novi predmeti su dostupni u prodavnici."];
         if(!seller || seller == "noSuchSeller")
         {
            seller = ["Scavengers","Бродяги","Бродяги", "Scavengers", "Scavengers", "Sakupljači"][lang];
         }
         return [{
            "notificationType":NotificationType.QUEST,
            "srpDemo":true,
            "hideTimer":15000,
            "status":QuestStatus.COMPLETED,
            "faction_id":Faction.SCAVENGERS,
            "quest_icon":1,
            "quest_label":quest,
            "label":Util.styledChar(LocalizationData.getName("quest_status_2"),"gold") + Util.styledChar("\n«" + quest + "»","gray_highlight"),
            "reward":[{
               "type":Reward.EXPERIENCE,
               "amount":1500
            },{
               "type":Reward.REPUTATION,
               "amount":150,
               "param":Faction.SCAVENGERS
            },{
               "type":Reward.MONEY,
               "amount":3000,
               "param":MoneyType.REGULAR
            }]
         },{
            "notificationType":NotificationType.PLAYER_RANK,
            "srpDemo":true,
            "type":ImportantMessageType.SKILL_POINT_REWARD,
            "hideTimer":15000,
            "skill_level":12,
            "reward_id":MoneyType.REGULAR,
            "reward_amount":5000,
            "label":Util.styledChar(LocalizationData.getName("level") + " " + "12" + " ","highlight") + LocalizationData.getName("achieved"),
            "reward":[{
               "type":Reward.MONEY,
               "amount":5000,
               "param":MoneyType.REGULAR
            },{
               "type":Reward.POINT,
               "amount":1
            }]
         },{
            "notificationType":NotificationType.FACTION_RANK,
            "srpDemo":true,
            "hideTimer":15000,
            "faction_id":Faction.SCAVENGERS,
            "level":3,
            "label":"«" + seller + "»" + rankText[lang]
         },{
            "notificationType":NotificationType.PLAIN,
            "srpDemo":true,
            "hideTimer":15000,
            "label":LocalizationData.getName("premium_post_reward_label")
         }];
      }
      
      protected function rpDemoAddRows(lobby:Object) : void
      {
         var rows:Array = null;
         var i:int = 0;
         var temp:Object = null;
         try
         {
            temp = lobby["notificationListTemp"];
            if(temp && temp.matchCalback is Function && temp.matchCalback != this.rpDemoTempMatch)
            {
               this._rpDemoTempCallback = temp.matchCalback;
               temp.matchCalback = this.rpDemoTempMatch;
            }
         }
         catch(err0:Error)
         {
         }
         rows = [{
            "notificationType":NotificationType.MATCH,
            "srpDemo":true,
            "hideTimer":5000,
            "match_id":484727998,
            "map_name":"Rudnya",
            "time":"18:42",
            "result":"38:41",
            "match_result":3,
            "reward":[{
               "type":Reward.RATING,
               "amount":12
            },{
               "type":Reward.EXPERIENCE,
               "amount":6500
            },{
               "type":Reward.REPUTATION,
               "amount":50,
               "param":Faction.SCAVENGERS
            },{
               "type":Reward.MONEY,
               "amount":1250,
               "param":MoneyType.REGULAR
            }]
         },{
            "notificationType":NotificationType.MATCH,
            "srpDemo":true,
            "hideTimer":5000,
            "match_id":1777910972,
            "map_name":"Vostok Radar Station",
            "time":"19:05",
            "result":"100:98",
            "match_result":1,
            "reward":null
         },{
            "notificationType":NotificationType.MATCH,
            "srpDemo":true,
            "hideTimer":5000,
            "match_id":0,
            "map_name":"Chemical Plant",
            "time":"19:31",
            "result":"1000:999",
            "match_result":3,
            "reward":null
         }];
         try
         {
            rows = rows.concat(this.rpDemoOtherRows());
         }
         catch(err1:Error)
         {
         }
         try
         {
            i = 0;
            while(i < rows.length)
            {
               lobby["add_notification"](rows[i]);
               i++;
            }
         }
         catch(err:Error)
         {
         }
      }
   }
}

