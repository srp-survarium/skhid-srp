package survarium.controls
{
   import com.greensock.TweenLite;
   import fl.transitions.easing.Strong;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.text.StyleSheet;
   import flash.utils.Dictionary;
   import flash.utils.Timer;
   import flash.utils.getDefinitionByName;
   import flash.utils.getTimer;
   import scaleform.clik.layout.LayoutData;
   import scaleform.clik.motion.Tween;
   
   public class NotificationList extends MovieClip
   {
      
      public var layoutData:LayoutData;
      
      protected var _notificationRednerer:Class;
      
      protected var _fastHideTime:int = 5500;
      
      protected var _longHideTime:int = 5500;
      
      protected var _isHiding:Boolean = true;
      
      protected var _timerRunning:Boolean = true;
      
      protected var _messages:Array = new Array();
      
      protected var _renderers:Array = new Array();
      
      protected var _targets:Array = new Array();
      
      protected var _style:StyleSheet;
      
      protected var _move_tween:Tween;
      
      protected var _item_tweens:Array;
      
      protected var _move:Number = 0;
      
      protected var _height:int = 0;
      
      protected var _direction:int = 1;
      
      protected var _hideTimer:Timer = new Timer(500);
      
      protected var _nextHideTime:int = 0;
      
      protected var _rendererTypes:Dictionary = new Dictionary();
      
      public var matchCalback:Function;
      
      public function NotificationList()
      {
         super();
         _height = this.height;
         _move_tween = new Tween(500,this,{move:0},{ease:Strong.easeOut});
         this._move_tween.paused = true;
         this._hideTimer.addEventListener(TimerEvent.TIMER,this.onTimerEvent);
      }
      
      public function registerRenderer(type:int, classRef:Class) : void
      {
         this._rendererTypes[type] = classRef;
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
         return 273;
      }
      
      override public function get width() : Number
      {
         return 290;
      }
      
      public function setNotificationRenderer(value:Class) : void
      {
         _notificationRednerer = value;
      }
      
      public function updateNotification(value:Object) : void
      {
         for(var i:uint = 0; i < this._renderers.length; i++)
         {
            if(this._renderers[i].data == value)
            {
               this._renderers[i].data = value;
               return;
            }
         }
      }
      
      public function addNotification(value:Object, useAlpha:Boolean = false) : void
      {
         if(value.hideTimer == undefined)
         {
            value.hideTimer = 5000;
         }
         var type:int = (value.notificationType);
         var itemRenderer:MovieClip;
         if(type == 3)
         {
            itemRenderer = new this._rendererTypes[type]();
            itemRenderer.addEventListener("showMessage",this.onMatchtMessage,false,0,true);
         }
         else if(value.quest_label)
         {
            itemRenderer = new this._rendererTypes[type]();
            itemRenderer.addEventListener("showMessage",this.onShowQuestMessage,false,0,true);
         }
         else
         {
            itemRenderer = new this._rendererTypes[type]();
            itemRenderer.addEventListener("showMessage",this.onRelatedMessage,false,0,true);
         }
         itemRenderer.closable = true;
         itemRenderer.data = value;
         itemRenderer.addEventListener(Event.CLOSE,this.onRendererClose,false,0,true);
         itemRenderer.validateNow();
         if(this._timerRunning == true)
         {
            itemRenderer.startTimer();
         }
         itemRenderer.alpha = 0;
         var rendererHeight:int = itemRenderer.height;
         itemRenderer.y = this._height - rendererHeight;
         addChild(itemRenderer);
         var i:int = (this._renderers.length);
         this._targets[i] = itemRenderer.y;
         trace("newTarget::",i,"|",this._targets[i],rendererHeight);
         for(i = 0; i < this._renderers.length; i++)
         {
            this._targets[i] -= rendererHeight * this._direction;
         }
         this._renderers.push(itemRenderer);
         this._messages.push(itemRenderer);
         this.clearTimer();
         this.validateRenderers();
         this.moveItems();
      }
      
      public function stopTimer() : void
      {
         _timerRunning = false;
         for(var i:uint = 0; i < this._renderers.length; i++)
         {
            this._renderers[i].stopTimer();
         }
      }
      
      public function startTimer() : void
      {
         _timerRunning = true;
         for(var i:uint = 0; i < this._renderers.length; i++)
         {
            this._renderers[i].startTimer();
         }
      }
      
      private function onRelatedMessage(e:Event) : void
      {
         trace("onRelatedMessage::",e.target,e.currentTarget);
         if(e.target.data && e.target.data.type == 2)
         {
            Message.showMessage("message_unlock_rank",e.target.data,false);
            this.hideItem(e.target as MovieClip);
         }
      }
      
      private function onShowQuestMessage(e:Event) : void
      {
         trace("onShowQuestMessage::",e.target,e.currentTarget);
         if(e.target is e.target.data.notificationType == 1)
         {
            Message.showMessage("quest_message",e.target.data);
            this.hideItem(e.target as MovieClip);
         }
      }
      
      private function onRendererClose(e:Event) : void
      {
         this.hideItem(e.target as MovieClip);
      }
      
      private function onTimerEvent(e:TimerEvent) : void
      {
         if(getTimer() >= this._nextHideTime)
         {
            this.hideItem();
         }
      }
      
      public function reset() : void
      {
         this._move_tween.reset();
         this._move_tween.paused = true;
      }
      
      public function clear(full:Boolean = false) : void
      {
         for(var i:uint = 0; i < this._renderers.length; i++)
         {
            removeChild(this._renderers[i]);
         }
         this._renderers.length = 0;
         this._targets.length = 0;
         if(full)
         {
            this._messages.length = 0;
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
      
      override public function toString() : String
      {
         return "[NotificationList " + name + "]";
      }
      
      protected function set move(value:Number) : void
      {
         for(var i:int = 0; i < this._renderers.length; i++)
         {
            if(this._renderers[i].y < this._targets[i])
            {
               this._renderers[i].y += 1;
            }
            else if(this._renderers[i].y > this._targets[i])
            {
               this._renderers[i].y -= 1;
            }
         }
         _move = value;
      }
      
      protected function get move() : Number
      {
         return this._move;
      }
      
      protected function clearTimer() : void
      {
         this._hideTimer.stop();
      }
      
      protected function hideItem(e:MovieClip = null) : void
      {
         trace("hideTimer::",e ? "byMovieClip: " + e.data.label : "byTime","delay:",this._hideTimer.delay,"renderers:",this._renderers.length,this._isHiding);
         var rendererIndex:int = 0;
         var renderer:MovieClip;
         if(e)
         {
            renderer = e;
            rendererIndex = this._renderers.indexOf(renderer);
            trace("rendererIndex",rendererIndex);
            if(rendererIndex == -1)
            {
               return;
            }
         }
         else
         {
            rendererIndex = -1;
            for(var j:int = 0; j < this._renderers.length; j++)
            {
               renderer = this._renderers[j];
               if(renderer.data && renderer.hideTimer != 0)
               {
                  rendererIndex = j;
                  break;
               }
            }
            renderer = this._renderers[rendererIndex];
         }
         if(!renderer || !renderer.parent)
         {
            return;
         }
         if(this._renderers.length > 1)
         {
         }
         trace("createTween");
         var rendererHeight:int = Math.ceil(renderer.height);
         renderer.clearTimer();
         TweenLite.killTweensOf(renderer);
         TweenLite.to(renderer,0.5,{
            y:this._targets[rendererIndex] = this._targets[rendererIndex] - 10 * this._direction,
            alpha:0,
            ease:Strong.easeOut,
            onComplete:renderer.clear
         });
         this._renderers.splice(rendererIndex,1);
         this._targets.splice(rendererIndex,1);
         trace("rendererIndex",rendererIndex,renderer.data.label);
         for(var i:int = 0; i < rendererIndex; i++)
         {
            this._targets[i] += rendererHeight * this._direction;
         }
         this.moveItems();
      }
      
      protected function moveItems() : void
      {
         for(var i:int = 0; i < this._renderers.length; i++)
         {
            var renderer:MovieClip = this._renderers[i];
            var tween:TweenLite = renderer.tween;
            if(tween)
            {
               tween.kill();
            }
            tween = TweenLite.to(renderer,0.5,{y:this._targets[i]});
            renderer.tween = tween;
         }
      }
      
      private function onMoveCoplete() : void
      {
         for(var i:int = 0; i < this._renderers.length; i++)
         {
            var renderer:MovieClip = this._renderers[i];
            trace("onMoveCoplete",renderer,renderer.y,this._targets[i],renderer.height);
         }
      }
      
      protected function items_height() : uint
      {
         var rendererHeight:uint = 0;
         for(var i:uint = 0; i < this._renderers.length; i++)
         {
            var renderer:MovieClip = this._renderers[i];
            if(renderer.parent)
            {
               rendererHeight += uint(renderer.height);
            }
         }
         return rendererHeight;
      }
      
      protected function messages_height() : uint
      {
         var rendererHeight:uint = 0;
         for(var i:uint = 0; i < this._messages.length; i++)
         {
            var renderer:MovieClip = this._messages[i];
            rendererHeight += uint(renderer.height);
         }
         return rendererHeight;
      }
      
      protected function validateRenderers() : void
      {
         while(1020 < this.messages_height())
         {
            this._messages.splice(0,1);
         }
         while(1020 < this.items_height())
         {
            this.hideItem();
         }
      }
   }
}

