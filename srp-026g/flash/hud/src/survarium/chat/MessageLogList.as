package survarium.chat
{
   import fl.transitions.easing.Strong;
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.text.StyleSheet;
   import flash.utils.Timer;
   import flash.utils.getTimer;
   import scaleform.clik.motion.Tween;
   
   public class MessageLogList extends MovieClip
   {
      
      protected var _fastHideTime:int = 3000;
      
      protected var _longHideTime:int = 7000;
      
      protected var _isHiding:Boolean = true;
      
      protected var _messages:Array = new Array();
      
      protected var _renderers:Array = new Array();
      
      protected var _targets:Array = new Array();
      
      protected var _style:StyleSheet;
      
      protected var _move_tween:Tween;
      
      protected var _item_tweens:Array;
      
      protected var _move:Number = 0;
      
      protected var _height:int = 0;
      
      protected var _hideTimer:Timer = new Timer(500);
      
      protected var _nextHideTime:int = 0;
      
      public function MessageLogList()
      {
         super();
         _height = height;
         _move_tween = new Tween(500,this,{move:0},{ease:Strong.easeOut});
         this._move_tween.paused = true;
         this._hideTimer.addEventListener(TimerEvent.TIMER,this.onTimerEvent);
      }
      
      public function get numMessages() : int
      {
         return this._renderers.length;
      }
      
      public function set styleSheet(value:StyleSheet) : void
      {
         _style = value;
      }
      
      public function setText(value:String) : void
      {
         var i:uint = 0;
         var itemRenderer:MovieClip;
         for(; i < this._renderers.length; i++)
         {
            removeChild(this._renderers[i]);
         }
         if(!this._renderers.length)
         {
            itemRenderer = new MessageRenderer();
         }
         else
         {
            itemRenderer = this._renderers[0];
         }
         this._renderers.length = 0;
         this._targets.length = 0;
         itemRenderer.textContainer.textField.styleSheet = this._style;
         itemRenderer.y = 0;
         var _start_index:Array = value.split("\n");
         while(_start_index.length > 10)
         {
            _start_index.shift();
         }
         value = _start_index.join("\n");
         itemRenderer.textField.htmlText = value;
         addChild(itemRenderer);
         this._targets[this._renderers.length] = itemRenderer.y;
         this._renderers.push(itemRenderer);
         this.clearTimer();
         this.validateRenderers();
      }
      
      public function addText(value:String) : void
      {
         this.addMessage(value,true);
      }
      
      public function addMessage(value:String, useAlpha:Boolean = false) : void
      {
         var itemRenderer:MovieClip = new MessageRenderer();
         itemRenderer.textContainer.textField.styleSheet = this._style;
         if(!useAlpha)
         {
            itemRenderer.textContainer.alpha = 0;
            itemRenderer.textContainer.textField.htmlText = value;
         }
         else
         {
            _start_index = value.split("\n");
            while(_start_index.length > 10)
            {
               _start_index.shift();
            }
            value = _start_index.join("\n");
            itemRenderer.textContainer.textField.htmlText = value;
         }
         itemRenderer.y = this.items_height();
         addChild(itemRenderer);
         var i:uint = this._renderers.length;
         var _start_index:Array;
         this._targets[i] = itemRenderer.y;
         if(!useAlpha)
         {
            new Tween(500,itemRenderer.textContainer,{alpha:1},{ease:Strong.easeOut});
         }
         this._renderers.push(itemRenderer);
         this._messages.push(itemRenderer);
         this.clearTimer();
         this.validateRenderers();
         this.startTimer(this._longHideTime);
      }
      
      private function startTimer(value:int) : void
      {
         trace("LogListStartTimer::",value);
         this._hideTimer.reset();
         this._hideTimer.start();
         _nextHideTime = getTimer() + value;
      }
      
      private function onTimerEvent(e:TimerEvent) : void
      {
         if(getTimer() >= this._nextHideTime)
         {
            this.hideItem(e);
         }
      }
      
      public function reset() : void
      {
         this._move_tween.reset();
         this._move_tween.paused = true;
         var hh:uint = 0;
         this._renderers.length = 0;
         for(var i:uint = 0; i < this._messages.length; i++)
         {
            var renderer:MovieClip;
            renderer = new MessageRenderer();
            renderer.textContainer.textField.styleSheet = this._style;
            renderer.textContainer.textField.htmlText = this._messages[i].textContainer.textField.htmlText;
            if(this._messages[i].parent)
            {
               removeChild(this._messages[i]);
            }
            addChild(renderer);
            renderer.y = hh;
            renderer.alpha = 1;
            this._targets[i] = renderer.y;
            this._renderers.push(renderer);
            this._messages[i] = renderer;
            hh += uint(renderer.textContainer.textField.textHeight);
         }
         this.startTimer(this._longHideTime);
      }
      
      public function showLastMessages() : void
      {
         if(this._messages.length == this._renderers.length || this._renderers[0] && this._renderers[0].fake)
         {
            return;
         }
         this._move_tween.reset();
         this._move_tween.paused = true;
         var renderer:MovieClip = new MessageRenderer();
         renderer.textContainer.textField.styleSheet = this._style;
         renderer.fake = true;
         var _str:String = "";
         for(var i:uint = 0; i < this._messages.length - this._renderers.length; i++)
         {
            _str += this._messages[i].textContainer.textField.htmlText;
         }
         renderer.textContainer.textField.htmlText = _str;
         var hh:uint = (renderer.textContainer.textField.textHeight);
         for(i = 0; i < this._renderers.length; i++)
         {
            this._targets[i] += hh;
            this._renderers[i].y = this._targets[i];
         }
         renderer.y = 0;
         addChild(renderer);
         this._targets.unshift(0);
         this._renderers.unshift(renderer);
         this.startTimer(this._longHideTime);
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
      
      public function stopHideout() : void
      {
         _isHiding = false;
         this._hideTimer.stop();
      }
      
      public function resumeHideout() : void
      {
         _isHiding = true;
         this.startTimer(this._longHideTime);
      }
      
      override public function toString() : String
      {
         return "[MessageLogList " + name + "]";
      }
      
      protected function set move(value:Number) : void
      {
         for(var i:int = 0; i < this._renderers.length; i++)
         {
            this._renderers[i].y = this._targets[i] + value;
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
      
      protected function hideItem(e:Event = null) : void
      {
         trace("hideTimer::",e ? "byEvent" : "noEvent","delay:",this._hideTimer.delay,"renderers:",this._renderers.length,this._isHiding);
         var i:uint = 0;
         var renderer:MovieClip = this._renderers[i];
         if(!renderer || !renderer.parent)
         {
            return;
         }
         if(this._renderers.length > 1)
         {
            this.startTimer(this._fastHideTime);
         }
         else
         {
            this.clearTimer();
         }
         var hh:int = (uint(this._renderers[i].textContainer.textField.textHeight));
         new Tween(500,this._renderers[i],{
            y:this._targets[i] = this._targets[i] - 10,
            alpha:0
         },{
            ease:Strong.easeOut,
            onComplete:this._renderers[i].clear
         });
         this._renderers.splice(i,1);
         this._targets.splice(i,1);
         for(i = 0; i < this._renderers.length; i++)
         {
            this._targets[i] -= hh;
         }
         _move += hh;
         this._move_tween.reset();
         this._move_tween.paused = true;
         _move_tween = new Tween(500,this,{move:0},{ease:Strong.easeOut});
      }
      
      protected function items_height() : uint
      {
         var hn:uint = 0;
         for(var i:uint = 0; i < this._renderers.length; i++)
         {
            var renderer:MovieClip;
            renderer = this._renderers[i];
            if(renderer.parent)
            {
               hn += uint(renderer.textContainer.textField.textHeight);
            }
         }
         return hn;
      }
      
      protected function messages_height() : uint
      {
         var hn:uint = 0;
         for(var i:uint = 0; i < this._messages.length; i++)
         {
            var renderer:MovieClip;
            renderer = this._messages[i];
            hn += uint(renderer.textContainer.textField.textHeight);
         }
         return hn;
      }
      
      protected function validateRenderers() : void
      {
         while(this._height < this.messages_height())
         {
            this._messages.splice(0,1);
         }
         while(this._height < this.items_height())
         {
            this.hideItem();
         }
      }
   }
}

