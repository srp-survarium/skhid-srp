package com.greensock.core
{
   import flash.display.Shape;
   import flash.events.Event;
   import flash.utils.getTimer;
   
   public class Animation
   {
      
      public static const version:String = "12.1.1";
      
      public static var ticker:Shape = new Shape();
      
      public static var _rootTimeline:SimpleTimeline;
      
      public static var _rootFramesTimeline:SimpleTimeline;
      
      protected static var _rootFrame:Number = -1;
      
      protected static var _tickEvent:Event = new Event("tick");
      
      protected static var _tinyNum:Number = 1e-10;
      
      protected var _onUpdate:Function;
      
      public var _delay:Number;
      
      public var _rawPrevTime:Number;
      
      public var _active:Boolean;
      
      public var _gc:Boolean;
      
      public var _initted:Boolean;
      
      public var _startTime:Number;
      
      public var _time:Number;
      
      public var _totalTime:Number;
      
      public var _duration:Number;
      
      public var _totalDuration:Number;
      
      public var _pauseTime:Number;
      
      public var _timeScale:Number;
      
      public var _reversed:Boolean;
      
      public var _timeline:SimpleTimeline;
      
      public var _dirty:Boolean;
      
      public var _paused:Boolean;
      
      public var _next:Animation;
      
      public var _prev:Animation;
      
      public var vars:Object;
      
      public var timeline:SimpleTimeline;
      
      public var data:*;
      
      public function Animation(duration:Number = 0, varsValue:Object = null)
      {
         super();
         vars = varsValue || {};
         if(this.vars._isGSVars)
         {
            vars = this.vars.vars;
         }
         _duration = _totalDuration = duration || 0;
         _delay = Number(this.vars.delay) || 0;
         _timeScale = 1;
         _totalTime = _time = 0;
         data = this.vars.data;
         _rawPrevTime = -1;
         if(_rootTimeline == null)
         {
            if(_rootFrame == -1)
            {
               _rootFrame = 0;
               _rootFramesTimeline = new SimpleTimeline();
               _rootTimeline = new SimpleTimeline();
               _rootTimeline._startTime = getTimer() / 1000;
               _rootFramesTimeline._startTime = 0;
               _rootTimeline._active = _rootFramesTimeline._active = true;
               ticker.addEventListener("enterFrame",_updateRoot,false,0,true);
            }
            else
            {
               return;
            }
         }
         var tl:SimpleTimeline = this.vars.useFrames ? _rootFramesTimeline : _rootTimeline;
         tl.add(this,tl._time);
         _reversed = this.vars.reversed == true;
         if(this.vars.paused)
         {
            this.paused(true);
         }
      }
      
      public static function _updateRoot(event:Event = null) : void
      {
         ++_rootFrame;
         _rootTimeline.render((getTimer() / 1000 - _rootTimeline._startTime) * _rootTimeline._timeScale,false,false);
         _rootFramesTimeline.render((_rootFrame - _rootFramesTimeline._startTime) * _rootFramesTimeline._timeScale,false,false);
         ticker.dispatchEvent(_tickEvent);
      }
      
      public function play(from:* = null, suppressEvents:Boolean = true) : *
      {
         if(arguments.length)
         {
            this.seek(from,suppressEvents);
         }
         this.reversed(false);
         return this.paused(false);
      }
      
      public function pause(atTime:* = null, suppressEvents:Boolean = true) : *
      {
         if(arguments.length)
         {
            this.seek(atTime,suppressEvents);
         }
         return this.paused(true);
      }
      
      public function resume(from:* = null, suppressEvents:Boolean = true) : *
      {
         if(arguments.length)
         {
            this.seek(from,suppressEvents);
         }
         return this.paused(false);
      }
      
      public function seek(time:*, suppressEvents:Boolean = true) : *
      {
         return this.totalTime(Number(time),suppressEvents);
      }
      
      public function restart(includeDelay:Boolean = false, suppressEvents:Boolean = true) : *
      {
         this.reversed(false);
         this.paused(false);
         return this.totalTime(includeDelay ? -this._delay : 0,suppressEvents,true);
      }
      
      public function reverse(from:* = null, suppressEvents:Boolean = true) : *
      {
         if(arguments.length)
         {
            this.seek(from || this.totalDuration(),suppressEvents);
         }
         this.reversed(true);
         return this.paused(false);
      }
      
      public function render(time:Number, suppressEvents:Boolean = false, force:Boolean = false) : void
      {
      }
      
      public function invalidate() : *
      {
         return this;
      }
      
      public function isActive() : Boolean
      {
         var tl:SimpleTimeline = this._timeline;
         var rawTime:Number;
         return tl == null || !this._gc && !this._paused && tl.isActive() && (rawTime = tl.rawTime()) >= this._startTime && rawTime < this._startTime + this.totalDuration() / this._timeScale;
      }
      
      public function _enabled(enabled:Boolean, ignoreTimeline:Boolean = false) : Boolean
      {
         _gc = !enabled;
         _active = Boolean(enabled && !this._paused && this._totalTime > 0 && this._totalTime < this._totalDuration);
         if(!ignoreTimeline)
         {
            if(enabled && this.timeline == null)
            {
               this._timeline.add(this,this._startTime - this._delay);
            }
            else if(!enabled && this.timeline != null)
            {
               this._timeline._remove(this,true);
            }
         }
         return false;
      }
      
      public function _kill(vars:Object = null, target:Object = null) : Boolean
      {
         return this._enabled(false,false);
      }
      
      public function kill(vars:Object = null, target:Object = null) : *
      {
         this._kill(vars,target);
         return this;
      }
      
      protected function _uncache(includeSelf:Boolean) : *
      {
         var tween:Animation = includeSelf ? this : this.timeline;
         while(tween)
         {
            tween._dirty = true;
            tween = tween.timeline;
         }
         return this;
      }
      
      protected function _swapSelfInParams(params:Array) : Array
      {
         var i:int = (params.length);
         var copy:Array = params.concat();
         while(--i > -1)
         {
            if(params[i] === "{self}")
            {
               copy[i] = this;
            }
         }
         return copy;
      }
      
      public function eventCallback(type:String, callback:Function = null, params:Array = null) : *
      {
         if(type == null)
         {
            return null;
         }
         if(type.substr(0,2) == "on")
         {
            if(arguments.length == 1)
            {
               return this.vars[type];
            }
            if(callback == null)
            {
               delete this.vars[type];
            }
            else
            {
               this.vars[type] = callback;
               this.vars[type + "Params"] = params is Array && params.join("").indexOf("{self}") !== -1 ? this._swapSelfInParams(params) : params;
            }
            if(type == "onUpdate")
            {
               _onUpdate = callback;
            }
         }
         return this;
      }
      
      public function delay(value:Number = NaN) : *
      {
         if(!arguments.length)
         {
            return this._delay;
         }
         if(this._timeline.smoothChildTiming)
         {
            this.startTime(this._startTime + value - this._delay);
         }
         _delay = value;
         return this;
      }
      
      public function duration(value:Number = NaN) : *
      {
         if(!arguments.length)
         {
            _dirty = false;
            return this._duration;
         }
         _duration = _totalDuration = value;
         this._uncache(true);
         if(this._timeline.smoothChildTiming)
         {
            if(this._time > 0)
            {
               if(this._time < this._duration)
               {
                  if(value != 0)
                  {
                     this.totalTime(this._totalTime * (value / this._duration),true);
                  }
               }
            }
         }
         return this;
      }
      
      public function totalDuration(value:Number = NaN) : *
      {
         _dirty = false;
         return !arguments.length ? this._totalDuration : this.duration(value);
      }
      
      public function time(value:Number = NaN, suppressEvents:Boolean = false) : *
      {
         if(!arguments.length)
         {
            return this._time;
         }
         if(this._dirty)
         {
            this.totalDuration();
         }
         if(value > this._duration)
         {
            value = this._duration;
         }
         return this.totalTime(value,suppressEvents);
      }
      
      public function totalTime(time:Number = NaN, suppressEvents:Boolean = false, uncapped:Boolean = false) : *
      {
         var tl:SimpleTimeline;
         if(!arguments.length)
         {
            return this._totalTime;
         }
         if(this._timeline)
         {
            if(time < 0 && !uncapped)
            {
               time += this.totalDuration();
            }
            if(this._timeline.smoothChildTiming)
            {
               if(this._dirty)
               {
                  this.totalDuration();
               }
               if(time > this._totalDuration && !uncapped)
               {
                  time = this._totalDuration;
               }
               tl = this._timeline;
               _startTime = (this._paused ? this._pauseTime : tl._time) - (!this._reversed ? time : this._totalDuration - time) / this._timeScale;
               if(!this._timeline._dirty)
               {
                  this._uncache(false);
               }
               if(tl._timeline != null)
               {
                  while(tl._timeline)
                  {
                     if(tl._timeline._time !== (tl._startTime + tl._totalTime) / tl._timeScale)
                     {
                        tl.totalTime(tl._totalTime,true);
                     }
                     tl = tl._timeline;
                  }
               }
            }
            if(this._gc)
            {
               this._enabled(true,false);
            }
            if(this._totalTime != time || this._duration === 0)
            {
               this.render(time,suppressEvents,false);
            }
         }
         return this;
      }
      
      public function progress(value:Number = NaN, suppressEvents:Boolean = false) : *
      {
         return !arguments.length ? this._time / this.duration() : this.totalTime(this.duration() * value,suppressEvents);
      }
      
      public function totalProgress(value:Number = NaN, suppressEvents:Boolean = false) : *
      {
         return !arguments.length ? this._time / this.duration() : this.totalTime(this.duration() * value,suppressEvents);
      }
      
      public function startTime(value:Number = NaN) : *
      {
         if(!arguments.length)
         {
            return this._startTime;
         }
         if(value != this._startTime)
         {
            _startTime = value;
            if(this.timeline)
            {
               if(this.timeline._sortChildren)
               {
                  this.timeline.add(this,value - this._delay);
               }
            }
         }
         return this;
      }
      
      public function timeScale(value:Number = NaN) : *
      {
         var t:Number;
         if(!arguments.length)
         {
            return this._timeScale;
         }
         value ||= 0.000001;
         if(this._timeline && this._timeline.smoothChildTiming)
         {
            t = this._pauseTime || this._pauseTime == 0 ? this._pauseTime : this._timeline._totalTime;
            _startTime = t - (t - this._startTime) * this._timeScale / value;
         }
         _timeScale = value;
         return this._uncache(false);
      }
      
      public function reversed(value:Boolean = false) : *
      {
         if(!arguments.length)
         {
            return this._reversed;
         }
         if(value != this._reversed)
         {
            _reversed = value;
            this.totalTime(this._timeline && !this._timeline.smoothChildTiming ? this.totalDuration() - this._totalTime : this._totalTime,true);
         }
         return this;
      }
      
      public function paused(value:Boolean = false) : *
      {
         var raw:Number;
         var elapsed:Number;
         if(!arguments.length)
         {
            return this._paused;
         }
         if(value != this._paused)
         {
            if(this._timeline)
            {
               raw = this._timeline.rawTime();
               elapsed = raw - this._pauseTime;
               if(!value && this._timeline.smoothChildTiming)
               {
                  _startTime += elapsed;
                  this._uncache(false);
               }
               _pauseTime = value ? raw : NaN;
               _paused = value;
               _active = !value && this._totalTime > 0 && this._totalTime < this._totalDuration;
               if(!value && elapsed != 0 && this._initted && this.duration() !== 0)
               {
                  this.render(this._timeline.smoothChildTiming ? this._totalTime : (raw - this._startTime) / this._timeScale,true,true);
               }
            }
         }
         if(this._gc && !value)
         {
            this._enabled(true,false);
         }
         return this;
      }
   }
}

