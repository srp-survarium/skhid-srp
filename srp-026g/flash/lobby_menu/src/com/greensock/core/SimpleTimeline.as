package com.greensock.core
{
   public class SimpleTimeline extends Animation
   {
      
      public var autoRemoveChildren:Boolean;
      
      public var smoothChildTiming:Boolean;
      
      public var _sortChildren:Boolean;
      
      public var _first:Animation;
      
      public var _last:Animation;
      
      public function SimpleTimeline(vars:Object = null)
      {
         super(0,vars);
         autoRemoveChildren = smoothChildTiming = true;
      }
      
      public function insert(child:*, position:* = 0) : *
      {
         return this.add(child,position || 0);
      }
      
      public function add(child:*, position:* = "+=0", align:String = "normal", stagger:Number = 0) : *
      {
         child._startTime = Number(position || 0) + child._delay;
         if(child._paused)
         {
            if(this != child._timeline)
            {
               child._pauseTime = child._startTime + (this.rawTime() - child._startTime) / child._timeScale;
            }
         }
         if(child.timeline)
         {
            child.timeline._remove(child,true);
         }
         child.timeline = child._timeline = this;
         if(child._gc)
         {
            child._enabled(true,true);
         }
         var prevTween:Animation = this._last;
         var st:Number;
         if(this._sortChildren)
         {
            st = child._startTime;
            while(prevTween && prevTween._startTime > st)
            {
               prevTween = prevTween._prev;
            }
         }
         if(prevTween)
         {
            child._next = prevTween._next;
            prevTween._next = Animation(child);
         }
         else
         {
            child._next = this._first;
            _first = Animation(child);
         }
         if(child._next)
         {
            child._next._prev = child;
         }
         else
         {
            _last = Animation(child);
         }
         child._prev = prevTween;
         if(_timeline)
         {
            _uncache(true);
         }
         return this;
      }
      
      public function _remove(tween:Animation, skipDisable:Boolean = false) : *
      {
         if(tween.timeline == this)
         {
            if(!skipDisable)
            {
               tween._enabled(false,true);
            }
            tween.timeline = null;
            if(tween._prev)
            {
               tween._prev._next = tween._next;
            }
            else if(this._first === tween)
            {
               _first = tween._next;
            }
            if(tween._next)
            {
               tween._next._prev = tween._prev;
            }
            else if(this._last === tween)
            {
               _last = tween._prev;
            }
            if(_timeline)
            {
               _uncache(true);
            }
         }
         return this;
      }
      
      override public function render(time:Number, suppressEvents:Boolean = false, force:Boolean = false) : void
      {
         var tween:Animation = this._first;
         var next:Animation;
         _totalTime = _time = _rawPrevTime = time;
         while(tween)
         {
            next = tween._next;
            if(tween._active || time >= tween._startTime && !tween._paused)
            {
               if(!tween._reversed)
               {
                  tween.render((time - tween._startTime) * tween._timeScale,suppressEvents,force);
               }
               else
               {
                  tween.render((!tween._dirty ? tween._totalDuration : tween.totalDuration()) - (time - tween._startTime) * tween._timeScale,suppressEvents,force);
               }
            }
            tween = next;
         }
      }
      
      public function rawTime() : Number
      {
         return _totalTime;
      }
   }
}

