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
   
   public class NotificationListScrollContent extends MovieClip
   {
      
      public static const REPLAY_DEMO:Boolean = false;
      
      public static const HISTORY_EXTRA:int = 100;
      
      public static const REPLAY_ASK:uint = 4294967041;
      
      public static const REPLAY_START:uint = 2200000000;
      
      public static const REPLAY_POLLS:int = 30;
      
      public static const REPLAY_PREPARE_MS:int = 180000;
      
      public static const REPLAY_DEMO_PREPARE_MS:int = 30000;
      
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
      
      override public function set x(param1:Number) : void
      {
         super.x = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function set y(param1:Number) : void
      {
         super.y = int(param1 * MainScene.scaleFactor) / MainScene.scaleFactor;
      }
      
      override public function get height() : Number
      {
         return 482;
      }
      
      override public function get width() : Number
      {
         return 420;
      }
      
      public function updateNotification(param1:Object) : void
      {
         var _loc2_:uint;
         while(_loc2_ < this._renderers.length)
         {
            if(this._renderers[_loc2_].data == param1)
            {
               this._renderers[_loc2_].data = param1;
               this.historyUpdated(param1);
               return;
            }
            _loc2_++;
         }
      }
      
      public function registerRenderer(param1:int, param2:Class) : void
      {
         this._rendererTypes[param1] = param2;
         this.historyRegistered(param1);
      }
      
      public function addNotification(param1:Object, param2:Boolean = false) : void
      {
         var _loc3_:int = (param1.notificationType);
         var _loc4_:MovieClip;
         if(_loc3_ == 3)
         {
            _loc4_ = new this._rendererTypes[_loc3_]();
            _loc4_.addEventListener("showMessage",this.onMatchtMessage,false,0,true);
         }
         else if(param1.quest_label)
         {
            _loc4_ = new this._rendererTypes[_loc3_]();
            _loc4_.addEventListener("showMessage",this.onShowQuestMessage,false,0,true);
         }
         else
         {
            _loc4_ = new this._rendererTypes[_loc3_]();
            _loc4_.addEventListener("showMessage",this.onRelatedMessage,false,0,true);
         }
         this.historyWiden(_loc4_,param1);
         _loc4_.data = param1;
         _loc4_.validateNow();
         this.historyDecorate(_loc4_);
         _loc4_.y = this._height;
         _height += _loc4_.height + 1;
         this._scrollContent.addContent(_loc4_);
         this._scrollContent.validateNow();
         trace("NSLC::addNotification",_loc4_.y,this._scrollContent.scrolChildrens.y,this._scrollContent.y);
         this._renderers.push(_loc4_);
         this.background.y = Math.min(Math.max(this._scrollContent.scrolChildrens.y,0),this.height - 5);
         this.background.height = this.height - this.background.y;
      }
      
      private function onRelatedMessage(param1:Event) : void
      {
         trace("onRelatedMessage::",param1.target,param1.currentTarget);
         if(param1.target.data && param1.target.data.type == 2)
         {
            Message.showMessage("message_unlock_rank",param1.target.data,false);
         }
      }
      
      private function onMatchtMessage(param1:Event) : void
      {
         if(this.historyDemoEvent(param1))
         {
            return;
         }
         trace("onMatchtMessage::",param1.target,param1.currentTarget);
         if(param1.target.data)
         {
            this.matchCalback(param1.target.data);
         }
      }
      
      private function onShowQuestMessage(param1:Event) : void
      {
         trace("onShowQuestMessage::",param1.target,param1.currentTarget);
         if(param1.target.data && param1.target.data.type == 3)
         {
            Message.showMessage("quest_message",param1.target.data);
         }
      }
      
      public function reset() : void
      {
      }
      
      public function clear(param1:Boolean = false) : void
      {
         var _loc2_:uint;
         while(_loc2_ < this._renderers.length)
         {
            removeChild(this._renderers[_loc2_]);
            _loc2_++;
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
         this._rpState = 0;
         this._rpMatch = 0;
         this._rpPolls = 0;
         this._rpStatus = "";
         this._rpHex = "";
         this._rpSize = 0;
         this._rpPlace = 0;
         this._rpPrepSeen = false;
         this._rpAskAt = 0;
         this._rpShownText = "";
         this._rpShownKey = -1;
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
         if(NotificationListScrollContent.REPLAY_DEMO && type == 3)
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
            if(data && int(data.notificationType) == 1 && data.icon == undefined)
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
            if(!data || int(data.notificationType) != 3)
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
         var label:String = this.rpText(0);
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
            return 1;
         }
         if(word == "Друзья")
         {
            return 2;
         }
         return 0;
      }
      
      protected function rpWords() : Array
      {
         var words:Array = [["Watch replay","Дивитися повтор","Смотреть повтор"],["DOWNLOAD AND WATCH","ЗАВАНТАЖИТИ Й ДИВИТИСЯ","СКАЧАТЬ И СМОТРЕТЬ"],["CLOSE","ЗАКРИТИ","ЗАКРЫТЬ"],["Asking the match server for the replay of this match…","Запитуємо в сервера матчу повтор цього матчу…","Запрашиваем у сервера матча повтор этого матча…"],[["The replay is being made from the match record. This happens the first time anyone watches this match and takes a little while.\nYou can close this window: the server keeps making it.\n\n",""],["Повтор створюється із запису матчу. Так буває, коли цей матч дивляться вперше, і це займає трохи часу.\nЦе вікно можна закрити: сервер продовжить його створювати.\n\n",""],["Повтор создаётся из записи матча. Так бывает, когда этот матч смотрят впервые, и это занимает немного времени.\nЭто окно можно закрыть: сервер продолжит его создавать.\n\n",""]],[["Replay of this match: ",".\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed."]
         ,["Повтор цього матчу: ",".\n\nВін завантажується із сервера матчу, який зберігає достовірний запис матчу. Відкриється друге вікно гри й відтворить його; лобі залишиться відкритим.\n\nУ повторі натисніть F11 для керування: гравці, камера, перемотування, швидкість."],["Повтор этого матча: ",".\n\nОн загружается с сервера матча, который хранит достоверную запись матча. Откроется второе окно игры и воспроизведёт его; лобби останется открытым.\n\nВ повторе нажмите F11 для управления: игроки, камера, перемотка, скорость."]],["The replay of this match is ready.\n\nIt is downloaded from the match server, which keeps the authoritative record of the match. A second game window opens and plays it; the lobby stays open.\n\nIn the replay press F11 for the controls: players, camera, seek, speed.","Повтор цього матчу готовий.\n\nВін завантажується із сервера матчу, який зберігає достовірний запис матчу. Відкриється друге вікно гри й відтворить його; лобі залишиться відкритим.\n\nУ повторі натисніть F11 для керування: гравці, камера, перемотування, швидкість."
         ,"Повтор этого матча готов.\n\nОн загружается с сервера матча, который хранит достоверную запись матча. Откроется второе окно игры и воспроизведёт его; лобби останется открытым.\n\nВ повторе нажмите F11 для управления: игроки, камера, перемотка, скорость."],["This match was not recorded: it had fewer than 6 players, or recording was off at the time.","Цей матч не записано: у ньому було менше 6 гравців або запис тоді був вимкнений.","Этот матч не записан: в нём было меньше 6 игроков или запись тогда была выключена."],["The replay of this match is no longer kept on the server.","Повтор цього матчу більше не зберігається на сервері.","Повтор этого матча больше не хранится на сервере."],["You can watch replays only of matches you played in.","Дивитися можна лише повтори матчів, у яких ви грали.","Смотреть можно только повторы матчей, в которых вы играли."],["Your game version cannot play replays. Update the game: start it again through the launcher.","Ваша версія гри не відтворює повтори. Оновіть гру: запустіть її знову через лаунчер."
         ,"Ваша версия игры не воспроизводит повторы. Обновите игру: запустите её снова через лаунчер."],["This replay was recorded by a game version that yours cannot play.","Цей повтор записала версія гри, яку ваша гра не може відтворити.","Этот повтор записан версией игры, которую ваша игра не может воспроизвести."],["The server is busy with other replays right now. Try again in a minute.","Сервер зараз зайнятий іншими повторами. Спробуйте ще раз за хвилину.","Сервер сейчас занят другими повторами. Попробуйте ещё раз через минуту."],["The server could not prepare the replay of this match.","Серверу не вдалося підготувати повтор цього матчу.","Серверу не удалось подготовить повтор этого матча."],["Replays are switched off on this server.","Повтори на цьому сервері вимкнено.","Повторы на этом сервере отключены."],["The server did not answer. Try again later.","Сервер не відповів. Спробуйте пізніше.","Сервер не ответил. Попробуйте позже."],["Starting the replay: a second game window will open; this lobby stays open. If something goes wrong, the updater shows what happened.\n\nIn the replay press F11 for the controls."
         ,"Запускаємо повтор: відкриється друге вікно гри, лобі залишиться відкритим. Якщо щось піде не так, програма оновлення покаже, що сталося.\n\nУ повторі натисніть F11 для керування.","Запускаем повтор: откроется второе окно игры, лобби останется открытым. Если что-то пойдёт не так, программа обновления покажет, что случилось.\n\nВ повторе нажмите F11 для управления."],["MB","МБ","МБ"],[["Place in line: ",". Waiting: "," s."],["Місце в черзі: ",". Очікування: "," с."],["Место в очереди: ",". Ожидание: "," с."]],[["Being made now. Waiting: "," s."],["Створюється зараз. Очікування: "," с."],["Создаётся сейчас. Ожидание: "," с."]],[["Waiting: "," s."],["Очікування: "," с."],["Ожидание: "," с."]],["Still being prepared. It keeps being made on the server; open Watch replay again in a minute.","Повтор ще готується. Сервер продовжує його створювати; відкрийте «Дивитися повтор» ще раз за хвилину.","Повтор ещё готовится. Сервер продолжает его создавать; откройте «Смотреть повтор» ещё раз через минуту."]];
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
               return 7 + i;
            }
            i++;
         }
         return -1;
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
         text = String(Math.floor(tenths / 10)) + (this.rpLang() != 0 ? "," : ".") + String(tenths % 10);
         return text + " " + this.rpText(17);
      }
      
      public function replayBegin(id:uint) : void
      {
         if(this._rpState == 1)
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
         this._rpState = 1;
         this.rpShow(3,false);
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
         if(this._rpState != 1)
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
               this._rpState = 3;
               this.rpShow(21,false);
               return;
            }
            this.rpShowPreparing();
         }
         else if(this._rpPolls > NotificationListScrollContent.REPLAY_POLLS)
         {
            this.rpStop();
            this._rpState = 3;
            this.rpShow(15,false);
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
         var key:int = -1;
         if(this._rpState != 1)
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
            this._rpPlace = int(Math.min(size,999));
            this.rpShowPreparing();
            return;
         }
         this._rpSize = size;
         if(status == "ready")
         {
            this.rpStop();
            this._rpState = 2;
            this.rpShow(this._rpSize > 0 ? 5 : 6,true);
            return;
         }
         key = this.rpFinalKey(status);
         if(key >= 0)
         {
            this.rpStop();
            this._rpState = 3;
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
            line = this.rpFill(18,[String(this._rpPlace),String(seconds)]);
         }
         else if(this._rpPlace == 1)
         {
            line = this.rpFill(19,[String(seconds)]);
         }
         else
         {
            line = this.rpFill(20,[String(seconds)]);
         }
         return this.rpFill(4,[line]);
      }
      
      protected function rpShowPreparing() : void
      {
         var text:String = this.rpPreparingText();
         var message:Object = this._rpDialog;
         var high:Number = 0;
         if(this._rpShownKey != 4 || !message || !message.textField)
         {
            this.rpShow(4,false);
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
               this.rpShow(4,false);
            }
         }
         catch(e:Error)
         {
            this.rpShow(4,false);
         }
      }
      
      protected function rpShow(key:int, download:Boolean) : void
      {
         var text:String = this.rpText(key);
         var data:Object = null;
         var message:Object = null;
         var format:TextFormat = null;
         if(key == 5)
         {
            text = this.rpFill(5,[this.rpSizeText(this._rpSize)]);
         }
         if(key == 4)
         {
            text = this.rpPreparingText();
         }
         this.rpDropDialog();
         this._rpShownText = text;
         data = {
            "text":text,
            "label_ok":(download ? this.rpText(1) : null),
            "label_cancel":this.rpText(2),
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
         this._rpShownKey = -1;
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
         this._rpState = 0;
         this._rpDialog = null;
         this._rpDialogData = null;
         this._rpShownKey = -1;
      }
      
      protected function onRpYes(data:Object = null) : void
      {
         var ready:Boolean = this._rpState == 2 && this._rpStatus == "ready" && this._rpHex == this.rpHex(this._rpMatch);
         this.rpStop();
         this._rpDialog = null;
         this._rpDialogData = null;
         this._rpShownKey = -1;
         if(!ready)
         {
            this._rpState = 0;
            return;
         }
         this._rpState = 4;
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
         this.rpShow(16,false);
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
         var quest:String = ["Victory contribution","Внесок у перемогу","Вклад в победу"][lang];
         var seller:String = GangShop.getSellerName(1);
         var rankText:Array = [": rank 3 reached. New items are available in the shop.",": досягнуто ранг 3. У крамниці з\'явилися нові предмети.",": достигнут ранг 3. В магазине появились новые предметы."];
         if(!seller || seller == "noSuchSeller")
         {
            seller = ["Scavengers","Бродяги","Бродяги"][lang];
         }
         return [{
            "notificationType":1,
            "srpDemo":true,
            "hideTimer":15000,
            "status":2,
            "faction_id":1,
            "quest_icon":1,
            "quest_label":quest,
            "label":Util.styledChar(LocalizationData.getName("quest_status_2"),"gold") + Util.styledChar("\n«" + quest + "»","gray_highlight"),
            "reward":[{
               "type":0,
               "amount":1500
            },{
               "type":1,
               "amount":150,
               "param":1
            },{
               "type":2,
               "amount":3000,
               "param":0
            }]
         },{
            "notificationType":9,
            "srpDemo":true,
            "type":8,
            "hideTimer":15000,
            "skill_level":12,
            "reward_id":0,
            "reward_amount":5000,
            "label":Util.styledChar(LocalizationData.getName("level") + " " + "12" + " ","highlight") + LocalizationData.getName("achieved"),
            "reward":[{
               "type":2,
               "amount":5000,
               "param":0
            },{
               "type":5,
               "amount":1
            }]
         },{
            "notificationType":8,
            "srpDemo":true,
            "hideTimer":15000,
            "faction_id":1,
            "level":3,
            "label":"«" + seller + "»" + rankText[lang]
         },{
            "notificationType":0,
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
            "notificationType":3,
            "srpDemo":true,
            "hideTimer":5000,
            "match_id":484727998,
            "map_name":"Rudnya",
            "time":"18:42",
            "result":"38:41",
            "match_result":3,
            "reward":[{
               "type":4,
               "amount":12
            },{
               "type":0,
               "amount":6500
            },{
               "type":1,
               "amount":50,
               "param":1
            },{
               "type":2,
               "amount":1250,
               "param":0
            }]
         },{
            "notificationType":3,
            "srpDemo":true,
            "hideTimer":5000,
            "match_id":1777910972,
            "map_name":"Vostok Radar Station",
            "time":"19:05",
            "result":"100:98",
            "match_result":1,
            "reward":null
         },{
            "notificationType":3,
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

