package com.greensock
{
   import com.greensock.core.Animation;
   import com.greensock.core.PropTween;
   import com.greensock.core.SimpleTimeline;
   import com.greensock.easing.Ease;
   import flash.display.Shape;
   import flash.events.Event;
   import flash.utils.Dictionary;
   import flash.utils.getDefinitionByName;
   
   public class TweenLite extends Animation
   {
      
      public static const version:String = "12.1.3";
      
      public static var defaultEase:Ease = new Ease(null,null,1,1);
      
      public static var defaultOverwrite:String = "auto";
      
      public static var ticker:Shape = Animation.ticker;
      
      public static var _plugins:Object = {};
      
      public static var _onPluginEvent:Function;
      
      protected static var _tweenLookup:Dictionary = new Dictionary(false);
      
      protected static var _reservedProps:Object = {
         ease:1,
         delay:1,
         overwrite:1,
         onComplete:1,
         onCompleteParams:1,
         onCompleteScope:1,
         useFrames:1,
         runBackwards:1,
         startAt:1,
         onUpdate:1,
         onUpdateParams:1,
         onUpdateScope:1,
         onStart:1,
         onStartParams:1,
         onStartScope:1,
         onReverseComplete:1,
         onReverseCompleteParams:1,
         onReverseCompleteScope:1,
         onRepeat:1,
         onRepeatParams:1,
         onRepeatScope:1,
         easeParams:1,
         yoyo:1,
         onCompleteListener:1,
         onUpdateListener:1,
         onStartListener:1,
         onReverseCompleteListener:1,
         onRepeatListener:1,
         orientToBezier:1,
         immediateRender:1,
         repeat:1,
         repeatDelay:1,
         data:1,
         paused:1,
         reversed:1
      };
      
      protected static var _overwriteLookup:Object;
      
      public var target:Object;
      
      public var ratio:Number;
      
      public var _propLookup:Object;
      
      public var _firstPT:PropTween;
      
      protected var _targets:Array;
      
      public var _ease:Ease;
      
      protected var _easeType:int;
      
      protected var _easePower:int;
      
      protected var _siblings:Array;
      
      protected var _overwrite:int;
      
      protected var _overwrittenProps:Object;
      
      protected var _notifyPluginsOfEnabled:Boolean;
      
      protected var _startAt:TweenLite;
      
      public function TweenLite(targetValue:Object, duration:Number, vars:Object)
      {
         var i:int;
         super(duration,vars);
         if(targetValue == null)
         {
            throw new Error("Cannot tween a null object. Duration: " + duration + ", data: " + this.data);
         }
         if(!_overwriteLookup)
         {
            _overwriteLookup = {
               none:0,
               all:1,
               auto:2,
               concurrent:3,
               allOnStart:4,
               preexisting:5,
               "true":1,
               "false":0
            };
            ticker.addEventListener("enterFrame",_dumpGarbage,false,-1,true);
         }
         ratio = 0;
         target = targetValue;
         _ease = defaultEase;
         _overwrite = !("overwrite" in this.vars) ? (_overwriteLookup[defaultOverwrite]) : (typeof this.vars.overwrite === "number" ? this.vars.overwrite >> 0 : (_overwriteLookup[this.vars.overwrite]));
         if(this.target is Array && typeof this.target[0] === "object")
         {
            _targets = this.target.concat();
            _propLookup = [];
            _siblings = [];
            i = (this._targets.length);
            while(--i > -1)
            {
               this._siblings[i] = _register(this._targets[i],this,false);
               if(this._overwrite == 1)
               {
                  if(this._siblings[i].length > 1)
                  {
                     _applyOverwrite(this._targets[i],this,null,1,this._siblings[i]);
                  }
               }
            }
         }
         else
         {
            _propLookup = {};
            _siblings = _tweenLookup[targetValue];
            if(this._siblings == null)
            {
               _siblings = _tweenLookup[targetValue] = [this];
            }
            else
            {
               this._siblings[this._siblings.length] = this;
               if(this._overwrite == 1)
               {
                  _applyOverwrite(targetValue,this,null,1,this._siblings);
               }
            }
         }
         if((this.vars.immediateRender) || duration == 0 && _delay == 0 && this.vars.immediateRender != false)
         {
            this.render(-_delay,false,true);
         }
      }
      
      public static function to(target:Object, duration:Number, vars:Object) : TweenLite
      {
         return new TweenLite(target,duration,vars);
      }
      
      public static function from(target:Object, duration:Number, vars:Object) : TweenLite
      {
         vars = _prepVars(vars,true);
         vars.runBackwards = true;
         return new TweenLite(target,duration,vars);
      }
      
      public static function fromTo(target:Object, duration:Number, fromVars:Object, toVars:Object) : TweenLite
      {
         toVars = _prepVars(toVars,true);
         fromVars = _prepVars(fromVars);
         toVars.startAt = fromVars;
         toVars.immediateRender = toVars.immediateRender != false && fromVars.immediateRender != false;
         return new TweenLite(target,duration,toVars);
      }
      
      protected static function _prepVars(vars:Object, immediateRender:Boolean = false) : Object
      {
         if((vars._isGSVars))
         {
            vars = vars.vars;
         }
         if(immediateRender && !("immediateRender" in vars))
         {
            vars.immediateRender = true;
         }
         return vars;
      }
      
      public static function delayedCall(delay:Number, callback:Function, params:Array = null, useFrames:Boolean = false) : TweenLite
      {
         return new TweenLite(callback,0,{
            delay:delay,
            onComplete:callback,
            onCompleteParams:params,
            onReverseComplete:callback,
            onReverseCompleteParams:params,
            immediateRender:false,
            useFrames:useFrames,
            overwrite:0
         });
      }
      
      public static function set(target:Object, vars:Object) : TweenLite
      {
         return new TweenLite(target,0,vars);
      }
      
      private static function _dumpGarbage(event:Event) : void
      {
         var i:int;
         var a:Array;
         var tgt:Object;
         if(_rootFrame / 60 >> 0 === _rootFrame / 60)
         {
            for(tgt in _tweenLookup)
            {
               a = _tweenLookup[tgt];
               i = (a.length);
               while(--i > -1)
               {
                  if((a[i]._gc))
                  {
                     a.splice(i,1);
                  }
               }
               if(a.length === 0)
               {
                  delete _tweenLookup[tgt];
               }
            }
         }
      }
      
      public static function killTweensOf(target:*, onlyActive:* = false, vars:Object = null) : void
      {
         if(typeof onlyActive === "object")
         {
            vars = onlyActive;
            onlyActive = false;
         }
         var a:Array = TweenLite.getTweensOf(target,onlyActive);
         var i:int = (a.length);
         while(--i > -1)
         {
            a[i]._kill(vars,target);
         }
      }
      
      public static function killDelayedCallsTo(func:Function) : void
      {
         killTweensOf(func);
      }
      
      public static function getTweensOf(target:*, onlyActive:Boolean = false) : Array
      {
         var i:int;
         var a:Array;
         var j:int;
         var t:TweenLite;
         if(target is Array && typeof target[0] != "string" && typeof target[0] != "number")
         {
            i = (target.length);
            a = [];
            while(--i > -1)
            {
               a = a.concat(getTweensOf(target[i],onlyActive));
            }
            i = (a.length);
            while(--i > -1)
            {
               t = a[i];
               j = i;
               while(--j > -1)
               {
                  if(t === a[j])
                  {
                     a.splice(i,1);
                  }
               }
            }
         }
         else
         {
            a = _register(target).concat();
            i = (a.length);
            while(--i > -1)
            {
               if((a[i]._gc) || onlyActive && !a[i].isActive())
               {
                  a.splice(i,1);
               }
            }
         }
         return a;
      }
      
      protected static function _register(target:Object, tween:TweenLite = null, scrub:Boolean = false) : Array
      {
         var a:Array = _tweenLookup[target];
         var i:int;
         if(a == null)
         {
            a = _tweenLookup[target] = [];
         }
         if((tween))
         {
            i = (a.length);
            a[i] = tween;
            if(scrub)
            {
               while(--i > -1)
               {
                  if(a[i] === tween)
                  {
                     a.splice(i,1);
                  }
               }
            }
         }
         return a;
      }
      
      protected static function _applyOverwrite(target:Object, tween:TweenLite, props:Object, mode:int, siblings:Array) : Boolean
      {
         var i:int;
         var changed:Boolean;
         var curTween:TweenLite;
         if(mode == 1 || mode >= 4)
         {
            l = (siblings.length);
            for(i = 0; i < l; i++)
            {
               curTween = siblings[i];
               if(curTween != tween)
               {
                  if(!curTween._gc)
                  {
                     if(curTween._enabled(false,false))
                     {
                        changed = true;
                     }
                  }
               }
               else if(mode == 5)
               {
                  break;
               }
            }
            return changed;
         }
         var startTime:Number = tween._startTime + 1e-10;
         var overlaps:Array = [];
         var oCount:int = 0;
         var zeroDur:Boolean = tween._duration == 0;
         var globalStart:Number;
         var l:int;
         i = (siblings.length);
         while(--i > -1)
         {
            curTween = siblings[i];
            if(curTween === tween || curTween._gc || curTween._paused)
            {
               continue;
            }
            {
               if(curTween._timeline != tween._timeline)
               {
                  globalStart ||= _checkOverlap(tween,0,zeroDur);
                  if(_checkOverlap(curTween,globalStart,zeroDur) === 0)
                  {
                     overlaps[oCount++] = curTween;
                  }
               }
               else if(curTween._startTime <= startTime)
               {
                  if(curTween._startTime + curTween.totalDuration() / curTween._timeScale > startTime)
                  {
                     if(!((zeroDur || !curTween._initted) && startTime - curTween._startTime <= 2e-10))
                     {
                        overlaps[oCount++] = curTween;
                     }
                  }
               }
            }
         }
         i = oCount;
         while(--i > -1)
         {
            curTween = overlaps[i];
            if(mode == 2)
            {
               if(curTween._kill(props,target))
               {
                  changed = true;
               }
            }
            if(mode !== 2 || !curTween._firstPT && curTween._initted)
            {
               if(curTween._enabled(false,false))
               {
                  changed = true;
               }
            }
         }
         return changed;
      }
      
      private static function _checkOverlap(tween:Animation, reference:Number, zeroDur:Boolean) : Number
      {
         var tl:SimpleTimeline = tween._timeline;
         var ts:Number = tl._timeScale;
         var t:Number = tween._startTime;
         var min:Number = 1e-10;
         while((tl._timeline))
         {
            t += tl._startTime;
            ts *= tl._timeScale;
            if(tl._paused)
            {
               return -100;
            }
            tl = tl._timeline;
         }
         t /= ts;
         return t > reference ? t - reference : (zeroDur && t == reference || !tween._initted && t - reference < 2 * min ? min : ((t = t + tween.totalDuration() / tween._timeScale / ts) > reference + min ? 0 : t - reference - min));
      }
      
      protected function _init() : void
      {
         var immediate:Boolean = (vars.immediateRender);
         var i:int;
         var initPlugins:Boolean;
         var pt:PropTween;
         var p:String;
         var copy:Object;
         if((vars.startAt))
         {
            if(this._startAt != null)
            {
               this._startAt.render(-1,true);
            }
            vars.startAt.overwrite = 0;
            vars.startAt.immediateRender = true;
            _startAt = new TweenLite(this.target,0,vars.startAt);
            if(immediate)
            {
               if(_time > 0)
               {
                  _startAt = null;
               }
               else if(_duration !== 0)
               {
                  return;
               }
            }
         }
         else if((vars.runBackwards) && _duration !== 0)
         {
            if(this._startAt != null)
            {
               this._startAt.render(-1,true);
               _startAt = null;
            }
            else
            {
               copy = {};
               for(p in vars)
               {
                  if(!(p in _reservedProps))
                  {
                     copy[p] = vars[p];
                  }
               }
               copy.overwrite = 0;
               copy.data = "isFromStart";
               _startAt = TweenLite.to(this.target,0,copy);
               if(!immediate)
               {
                  this._startAt.render(-1,true);
               }
               else if(_time === 0)
               {
                  return;
               }
            }
         }
         if(vars.ease is Ease)
         {
            _ease = vars.easeParams is Array ? vars.ease.config.apply(vars.ease,vars.easeParams) : vars.ease;
         }
         else if(typeof vars.ease === "function")
         {
            _ease = new Ease(vars.ease,vars.easeParams);
         }
         else
         {
            _ease = defaultEase;
         }
         _easeType = this._ease._type;
         _easePower = this._ease._power;
         _firstPT = null;
         if((this._targets))
         {
            i = (this._targets.length);
            while(--i > -1)
            {
               if(this._initProps(this._targets[i],this._propLookup[i] = {},this._siblings[i],(this._overwrittenProps) ? this._overwrittenProps[i] : null))
               {
                  initPlugins = true;
               }
            }
         }
         else
         {
            initPlugins = this._initProps(this.target,this._propLookup,this._siblings,this._overwrittenProps);
         }
         if(initPlugins)
         {
            _onPluginEvent("_onInitAllProps",this);
         }
         if((this._overwrittenProps))
         {
            if(this._firstPT == null)
            {
               if(typeof this.target !== "function")
               {
                  this._enabled(false,false);
               }
            }
         }
         if((vars.runBackwards))
         {
            pt = this._firstPT;
            while((pt))
            {
               pt.s += pt.c;
               pt.c = -pt.c;
               pt = pt._next;
            }
         }
         _onUpdate = vars.onUpdate;
         _initted = true;
      }
      
      protected function _initProps(target:Object, propLookup:Object, siblings:Array, overwrittenProps:Object) : Boolean
      {
         var vars:Object = this.vars;
         var p:String;
         var i:int;
         var initPlugins:Boolean;
         var plugin:Object;
         var val:Object;
         if(target == null)
         {
            return false;
         }
         for(p in vars)
         {
            val = vars[p];
            if(p in _reservedProps)
            {
               if(val is Array)
               {
                  if(val.join("").indexOf("{self}") !== -1)
                  {
                     vars[p] = _swapSelfInParams(val as Array);
                  }
               }
            }
            else if(p in _plugins && ((plugin = new _plugins[p]())._onInitTween(target,val,this)))
            {
               _firstPT = new PropTween(plugin,"setRatio",0,1,p,true,this._firstPT,plugin._priority);
               i = (plugin._overwriteProps.length);
               while(--i > -1)
               {
                  propLookup[plugin._overwriteProps[i]] = this._firstPT;
               }
               if((plugin._priority) || "_onInitAllProps" in plugin)
               {
                  initPlugins = true;
               }
               if("_onDisable" in plugin || "_onEnable" in plugin)
               {
                  _notifyPluginsOfEnabled = true;
               }
            }
            else
            {
               _firstPT = propLookup[p] = new PropTween(target,p,0,1,p,false,this._firstPT);
               this._firstPT.s = !this._firstPT.f ? Number(target[p]) : (target[(p.indexOf("set")) || !("get" + p.substr(3) in target) ? p : "get" + p.substr(3)]());
               this._firstPT.c = typeof val === "number" ? Number(val) - this._firstPT.s : (typeof val === "string" && val.charAt(1) === "=" ? int(val.charAt(0) + "1") * Number(val.substr(2)) : Number(val) || 0);
            }
         }
         if((overwrittenProps))
         {
            if(this._kill(overwrittenProps,target))
            {
               return this._initProps(target,propLookup,siblings,overwrittenProps);
            }
         }
         if(this._overwrite > 1)
         {
            if(this._firstPT != null)
            {
               if(siblings.length > 1)
               {
                  if(_applyOverwrite(target,this,propLookup,this._overwrite,siblings))
                  {
                     this._kill(propLookup,target);
                     return this._initProps(target,propLookup,siblings,overwrittenProps);
                  }
               }
            }
         }
         return initPlugins;
      }
      
      override public function render(time:Number, suppressEvents:Boolean = false, force:Boolean = false) : void
      {
         var isComplete:Boolean;
         var callback:String;
         var pt:PropTween;
         var rawPrevTime:Number;
         var prevTime:Number = _time;
         var r:Number;
         if(time >= _duration)
         {
            _totalTime = _time = _duration;
            ratio = this._ease._calcEnd ? this._ease.getRatio(1) : 1;
            if(!_reversed)
            {
               isComplete = true;
               callback = "onComplete";
            }
            if(_duration == 0)
            {
               rawPrevTime = _rawPrevTime;
               if(time === 0 || rawPrevTime < 0 || rawPrevTime === _tinyNum)
               {
                  if(rawPrevTime !== time)
                  {
                     force = true;
                     if(rawPrevTime > _tinyNum)
                     {
                        callback = "onReverseComplete";
                     }
                  }
               }
               _rawPrevTime = rawPrevTime = !suppressEvents || time !== 0 ? time : _tinyNum;
            }
         }
         else if(time < 1e-7)
         {
            _totalTime = _time = 0;
            ratio = this._ease._calcEnd ? this._ease.getRatio(0) : 0;
            if(prevTime != 0 || _duration == 0 && _rawPrevTime > _tinyNum)
            {
               callback = "onReverseComplete";
               isComplete = _reversed;
            }
            if(time < 0)
            {
               _active = false;
               if(_duration == 0)
               {
                  if(_rawPrevTime >= 0)
                  {
                     force = true;
                  }
                  _rawPrevTime = rawPrevTime = !suppressEvents || time !== 0 ? time : _tinyNum;
               }
            }
            else if(!_initted)
            {
               force = true;
            }
         }
         else
         {
            _totalTime = _time = time;
            if((this._easeType))
            {
               r = time / _duration;
               if(this._easeType == 1 || this._easeType == 3 && r >= 0.5)
               {
                  r = 1 - r;
               }
               if(this._easeType == 3)
               {
                  r *= 2;
               }
               if(this._easePower == 1)
               {
                  r *= r;
               }
               else if(this._easePower == 2)
               {
                  r *= r * r;
               }
               else if(this._easePower == 3)
               {
                  r *= r * r * r;
               }
               else if(this._easePower == 4)
               {
                  r *= r * r * r * r;
               }
               if(this._easeType == 1)
               {
                  ratio = 1 - r;
               }
               else if(this._easeType == 2)
               {
                  ratio = r;
               }
               else if(time / _duration < 0.5)
               {
                  ratio = r / 2;
               }
               else
               {
                  ratio = 1 - r / 2;
               }
            }
            else
            {
               ratio = this._ease.getRatio(time / _duration);
            }
         }
         if(_time == prevTime && !force)
         {
            return;
         }
         if(!_initted)
         {
            this._init();
            if(!_initted || _gc)
            {
               return;
            }
            if((_time) && !isComplete)
            {
               ratio = this._ease.getRatio(_time / _duration);
            }
            else if(isComplete && this._ease._calcEnd)
            {
               ratio = this._ease.getRatio(_time === 0 ? 0 : 1);
            }
         }
         if(!_active)
         {
            if(!_paused && _time !== prevTime && time >= 0)
            {
               _active = true;
            }
         }
         if(prevTime == 0)
         {
            if(this._startAt != null)
            {
               if(time >= 0)
               {
                  this._startAt.render(time,suppressEvents,force);
               }
               else if(!callback)
               {
                  callback = "_dummyGS";
               }
            }
            if((vars.onStart))
            {
               if(_time != 0 || _duration == 0)
               {
                  if(!suppressEvents)
                  {
                     vars.onStart.apply(null,vars.onStartParams);
                  }
               }
            }
         }
         pt = this._firstPT;
         while((pt))
         {
            if(pt.f)
            {
               pt.t[pt.p](pt.c * this.ratio + pt.s);
            }
            else
            {
               pt.t[pt.p] = pt.c * this.ratio + pt.s;
            }
            pt = pt._next;
         }
         if(_onUpdate != null)
         {
            if(time < 0 && this._startAt != null && _startTime != 0)
            {
               this._startAt.render(time,suppressEvents,force);
            }
            if(!suppressEvents)
            {
               if(_time !== prevTime || isComplete)
               {
                  _onUpdate.apply(null,vars.onUpdateParams);
               }
            }
         }
         if((callback))
         {
            if(!_gc)
            {
               if(time < 0 && this._startAt != null && _onUpdate == null && _startTime != 0)
               {
                  this._startAt.render(time,suppressEvents,force);
               }
               if(isComplete)
               {
                  if(_timeline.autoRemoveChildren)
                  {
                     this._enabled(false,false);
                  }
                  _active = false;
               }
               if(!suppressEvents)
               {
                  if((vars[callback]))
                  {
                     vars[callback].apply(null,vars[callback + "Params"]);
                  }
               }
               if(_duration === 0 && _rawPrevTime === _tinyNum && rawPrevTime !== _tinyNum)
               {
                  _rawPrevTime = 0;
               }
            }
         }
      }
      
      override public function _kill(vars:Object = null, target:Object = null) : Boolean
      {
         var i:int;
         var overwrittenProps:Object;
         var p:String;
         var pt:PropTween;
         var propLookup:Object;
         var changed:Boolean;
         var killProps:Object;
         var record:Boolean;
         if(vars === "all")
         {
            vars = null;
         }
         if(vars == null)
         {
            if(target == null || target == this.target)
            {
               return this._enabled(false,false);
            }
         }
         target = target || this._targets || this.target;
         if(target is Array && typeof target[0] === "object")
         {
            i = (target.length);
            while(--i > -1)
            {
               if(this._kill(vars,target[i]))
               {
                  changed = true;
               }
            }
         }
         else
         {
            if((this._targets))
            {
               i = (this._targets.length);
               while(--i > -1)
               {
                  if(target === this._targets[i])
                  {
                     propLookup = this._propLookup[i] || {};
                     _overwrittenProps = this._overwrittenProps || [];
                     overwrittenProps = this._overwrittenProps[i] = (vars) ? this._overwrittenProps[i] || {} : "all";
                     break;
                  }
               }
            }
            else
            {
               if(target !== this.target)
               {
                  return false;
               }
               propLookup = this._propLookup;
               overwrittenProps = _overwrittenProps = (vars) ? this._overwrittenProps || {} : "all";
            }
            if((propLookup))
            {
               killProps = vars || propLookup;
               record = vars != overwrittenProps && overwrittenProps != "all" && vars != propLookup && (typeof vars != "object" || vars._tempKill != true);
               for(p in killProps)
               {
                  pt = propLookup[p];
                  if(pt != null)
                  {
                     if(pt.pg && (pt.t._kill(killProps)))
                     {
                        changed = true;
                     }
                     if(!pt.pg || pt.t._overwriteProps.length === 0)
                     {
                        if((pt._prev))
                        {
                           pt._prev._next = pt._next;
                        }
                        else if(pt == this._firstPT)
                        {
                           _firstPT = pt._next;
                        }
                        if((pt._next))
                        {
                           pt._next._prev = pt._prev;
                        }
                        pt._next = pt._prev = null;
                     }
                     delete propLookup[p];
                  }
                  if(record)
                  {
                     overwrittenProps[p] = 1;
                  }
               }
               if(this._firstPT == null && _initted)
               {
                  this._enabled(false,false);
               }
            }
         }
         return changed;
      }
      
      override public function invalidate() : *
      {
         if(this._notifyPluginsOfEnabled)
         {
            _onPluginEvent("_onDisable",this);
         }
         _firstPT = null;
         _overwrittenProps = null;
         _onUpdate = null;
         _startAt = null;
         _initted = _active = _notifyPluginsOfEnabled = false;
         _propLookup = (this._targets) ? {} : [];
         return this;
      }
      
      override public function _enabled(enabled:Boolean, ignoreTimeline:Boolean = false) : Boolean
      {
         var i:int;
         if(enabled && _gc)
         {
            if((this._targets))
            {
               i = (this._targets.length);
               while(--i > -1)
               {
                  this._siblings[i] = _register(this._targets[i],this,true);
               }
            }
            else
            {
               _siblings = _register(this.target,this,true);
            }
         }
         super._enabled(enabled,ignoreTimeline);
         if(this._notifyPluginsOfEnabled)
         {
            if(this._firstPT != null)
            {
               return _onPluginEvent(enabled ? "_onEnable" : "_onDisable",this);
            }
         }
         return false;
      }
   }
}

