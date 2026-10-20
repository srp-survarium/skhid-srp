package scaleform.clik.motion
{
   import flash.display.DisplayObject;
   import flash.display.Shape;
   import flash.events.Event;
   import flash.geom.Matrix;
   import flash.geom.Transform;
   import flash.utils.getTimer;
   
   public class Tween
   {
      
      protected static var ticker:Shape = new Shape();
      
      protected static var workingMatrix:Matrix = new Matrix();
      
      protected static var firstTween:Tween;
      
      protected static var lastTime:uint = getTimer();
      
      protected static var degToRad:Number = 1 / 180 * Math.PI;
      
      public static var propsDO:Object = {
         x:true,
         y:true,
         rotation:true,
         scaleX:true,
         scaleY:true,
         alpha:true
      };
      
      ticker.addEventListener(Event.ENTER_FRAME,Tween.tick,false,0,true);
      
      public var target:Object;
      
      public var duration:Number;
      
      public var ease:Function;
      
      public var easeParam:Object;
      
      public var onComplete:Function;
      
      public var onChange:Function;
      
      public var data:Object;
      
      public var nextTween:Tween;
      
      public var frameBased:Boolean = false;
      
      public var delay:Number = 0;
      
      public var loop:Boolean = false;
      
      public var fastTransform:Boolean = true;
      
      protected var invalid:Boolean;
      
      protected var next:Tween;
      
      protected var prev:Tween;
      
      protected var _position:Number = 0;
      
      protected var _paused:Boolean = true;
      
      protected var startMatrix:Matrix;
      
      protected var deltaMatrix:Matrix;
      
      protected var transform:Transform;
      
      protected var targetDO:DisplayObject;
      
      protected var firstProp:Prop;
      
      public var props:Object;
      
      public function Tween(durationValue:Number, targetValue:Object = null, propsValue:Object = null, quickSet:Object = null)
      {
         super();
         duration = durationValue;
         target = targetValue;
         if(targetValue is DisplayObject)
         {
            targetDO = DisplayObject(targetValue);
            transform = this.targetDO.transform;
         }
         props = propsValue;
         if(quickSet)
         {
            this.quickSet(quickSet);
         }
         if(quickSet == null || quickSet.paused == null)
         {
            paused = false;
         }
      }
      
      protected static function tick(evt:Event) : void
      {
         var t:Number = getTimer();
         var delta:Number = t - lastTime;
         lastTime = t;
         var tween:Tween = firstTween;
         while(tween)
         {
            tween.updatePosition(tween.frameBased ? 1 : delta);
            tween = tween.next;
         }
      }
      
      protected static function removeTween(tween:Tween) : void
      {
         if(tween.prev)
         {
            tween.prev.next = tween.next;
         }
         if(tween.next)
         {
            tween.next.prev = tween.prev;
         }
         if(tween == firstTween)
         {
            firstTween = tween.next;
         }
         tween.prev = tween.next = null;
      }
      
      public static function removeAllTweens() : void
      {
         firstTween = null;
      }
      
      public function set position(value:Number) : void
      {
         this.updatePosition(value + this.delay - this._position);
      }
      
      public function get position() : Number
      {
         return this._position - this.delay;
      }
      
      public function get paused() : Boolean
      {
         return this._paused;
      }
      
      public function set paused(value:Boolean) : void
      {
         if(value == this._paused)
         {
            return;
         }
         _paused = value;
         if(value)
         {
            removeTween(this);
         }
         else
         {
            if(firstTween)
            {
               firstTween.prev = this;
               next = firstTween;
            }
            firstTween = this;
            if(this._position >= this.delay + this.duration)
            {
               _position = 0;
            }
         }
      }
      
      public function reset() : void
      {
         _position = 0;
      }
      
      public function quickSet(props:Object) : void
      {
         var name:String;
         for(name in props)
         {
            this[name] = props[name];
         }
      }
      
      protected function constructProp(name:String) : Prop
      {
         var prop:Prop = new Prop();
         prop.name = name;
         prop.prev = null;
         if(this.firstProp)
         {
            this.firstProp.prev = prop;
         }
         prop.next = this.firstProp;
         return firstProp = prop;
      }
      
      protected function init() : void
      {
         firstProp = null;
         var useMatrix:Boolean = false;
         var name:String;
         var prop:Prop;
         for(name in this.props)
         {
            if(this.fastTransform && this.transform && propsDO[name])
            {
               useMatrix = true;
            }
            else
            {
               prop = this.constructProp(name);
               prop.delta = this.props[name] - (prop.start = this.target[name]);
            }
         }
         if(useMatrix)
         {
            startMatrix = new Matrix(this.targetDO.scaleX,this.targetDO.rotation * degToRad,this.targetDO.alpha,this.targetDO.scaleY,this.targetDO.x,this.targetDO.y);
            deltaMatrix = new Matrix(isNaN(this.props.scaleX) ? 0 : this.props.scaleX - this.startMatrix.a,isNaN(this.props.rotation) ? 0 : (this.props.rotation - this.targetDO.rotation) * degToRad,isNaN(this.props.alpha) ? 0 : this.props.alpha - this.startMatrix.c,isNaN(this.props.scaleY) ? 0 : this.props.scaleY - this.startMatrix.d,isNaN(this.props.x) ? 0 : this.props.x - this.startMatrix.tx,isNaN(this.props.y) ? 0 : this.props.y - this.startMatrix.ty);
         }
         props = null;
      }
      
      protected function updatePosition(value:Number) : void
      {
         if(this.target == null)
         {
            paused = true;
            complete = true;
            return;
         }
         _position += value;
         if(this._position <= this.delay)
         {
            return;
         }
         if(this.props)
         {
            this.init();
         }
         var ratio:Number = (this._position - this.delay) / this.duration;
         var complete:Boolean = ratio >= 1;
         if(complete)
         {
            ratio = 1;
            _position = this.duration + this.delay;
         }
         if(this.ease != null)
         {
            ratio = this.easeParam == null ? (this.ease(ratio,0,1,1)) : (this.ease(ratio,0,1,1,this.easeParam));
         }
         if(this.startMatrix)
         {
            r = this.startMatrix.b + this.deltaMatrix.b * ratio;
            if(r)
            {
               c = Math.cos(r);
               s = Math.sin(r);
            }
            else
            {
               c = 1;
               s = 0;
            }
            workingMatrix.a = c * this.startMatrix.a + this.deltaMatrix.a * ratio;
            workingMatrix.b = s;
            workingMatrix.c = -s;
            workingMatrix.d = c * this.startMatrix.d + this.deltaMatrix.d * ratio;
            workingMatrix.tx = this.startMatrix.tx + this.deltaMatrix.tx * ratio;
            workingMatrix.ty = this.startMatrix.ty + this.deltaMatrix.ty * ratio;
            this.transform.matrix = workingMatrix;
            if(this.deltaMatrix.c)
            {
               this.targetDO.alpha = this.startMatrix.c + this.deltaMatrix.c * ratio;
            }
         }
         var prop:Prop = this.firstProp;
         var r:Number;
         var c:Number;
         var s:Number;
         while(prop)
         {
            this.target[prop.name] = prop.start + prop.delta * ratio;
            prop = prop.next;
         }
         if(this.onChange != null)
         {
            this.onChange(this);
         }
         if(complete)
         {
            if(this.loop)
            {
               this.reset();
            }
            else
            {
               paused = true;
            }
            if(this.nextTween)
            {
               this.nextTween.paused = false;
            }
            if(this.onComplete != null)
            {
               this.onComplete(this);
            }
         }
      }
   }
}

final class Prop
{
   
   public var next:Prop;
   
   public var prev:Prop;
   
   public var name:String;
   
   public var start:Number;
   
   public var delta:Number;
   
   public function Prop()
   {
      super();
   }
}
