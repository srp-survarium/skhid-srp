package survarium.controls
{
   import fl.transitions.easing.Strong;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.StatusIndicator;
   import scaleform.clik.motion.Tween;
   import scaleform.clik.utils.Constraints;
   
   public class ProgressIndicator extends StatusIndicator
   {
      
      public var newFill:Sprite;
      
      public var filled:Sprite;
      
      public var _progressTx:TextField;
      
      public var animated:Boolean = true;
      
      public var back:Sprite;
      
      public var golow:MovieClip;
      
      public var golow_tx:MovieClip;
      
      protected var _progress:Number = 0;
      
      protected var _progressShift:Number = 0;
      
      protected var _size:Number;
      
      protected var _mask:MovieClip;
      
      protected var _mask_new:MovieClip;
      
      protected var _label:String;
      
      protected var _tween:Tween;
      
      public function ProgressIndicator()
      {
         super();
         this.newFill.visible = false;
         _mask = new MovieClip();
         this._mask.graphics.clear();
         this._mask.graphics.beginFill(0,1);
         this._mask.graphics.drawRect(0,0,this.filled.width / this.filled.scaleX,this.filled.height);
         this._mask.graphics.endFill();
         this.filled.addChild(this._mask);
         this.filled.mask = this._mask;
         _mask_new = new MovieClip();
         this._mask_new.graphics.clear();
         this._mask_new.graphics.beginFill(0,1);
         this._mask_new.graphics.drawRect(0,0,this.newFill.width / this.newFill.scaleX,this.newFill.height);
         this._mask_new.graphics.endFill();
         this.newFill.addChild(this._mask_new);
         this.newFill.mask = this._mask_new;
         addEventListener(Event.RESIZE,this.onResize);
         this.onResize();
      }
      
      public function get progressLen() : Number
      {
         return this._progressShift;
      }
      
      public function set label(value:String) : void
      {
         _label = value;
         if(this.golow)
         {
            (this.golow._progressTx as TextField).autoSize = TextFieldAutoSize.CENTER;
            this.golow._progressTx.text = value;
         }
      }
      
      public function set progress(value:Number) : void
      {
         if(value)
         {
            this.newFill.visible = true;
         }
         else
         {
            this.newFill.visible = false;
         }
         _progress = value;
         _progressShift = 0;
         invalidate(InvalidationType.DATA);
      }
      
      public function get progress() : Number
      {
         return this._progress;
      }
      
      public function set progressLen(value:Number) : void
      {
         _progressShift = value;
         this.updatePosition();
      }
      
      public function showProgress(show:Boolean) : void
      {
         this.golow_tx.visible = show;
      }
      
      override protected function configUI() : void
      {
         constraints = new Constraints(this,ConstrainMode.REFLOW);
         constraints.addElement("back",this.back,Constraints.LEFT | Constraints.TOP | Constraints.BOTTOM | Constraints.RIGHT);
         if(this.golow_tx)
         {
            constraints.addElement("golow_tx",this.golow_tx,Constraints.RIGHT | Constraints.CENTER_V);
         }
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SIZE))
         {
            if(this.golow_tx || this.golow)
            {
               constraints.update(width,height);
            }
         }
         if(isInvalid(InvalidationType.DATA) && this._progress)
         {
            if(this._tween)
            {
               this._tween.paused = true;
               this._tween.reset();
               this._tween.data = null;
               this._tween.target = null;
            }
            if(this.animated)
            {
               _tween = new Tween(5000,this,{progressLen:this._progress},{
                  paused:false,
                  ease:Strong.easeOut,
                  loop:false,
                  onComplete:this.clearTween
               });
            }
         }
         else if(isInvalid(InvalidationType.DATA))
         {
            if(this._tween)
            {
               this.clearTween();
            }
            _progressShift = 0;
            this.updatePosition();
         }
      }
      
      protected function clearTween() : void
      {
         _progress = 0;
         if(this._tween)
         {
            this._tween.paused = true;
            this._tween.reset();
            this._tween.data = null;
            this._tween.target = null;
            _tween = null;
         }
      }
      
      protected function onResize(event:Event = null) : void
      {
         var angle:Number = rotation * Math.PI / 180;
         var cosA:Number = Math.abs(Math.cos(angle));
         var sinA:Number = Math.abs(Math.sin(angle));
         _size = (this.width * cosA - this.height * sinA) / (Math.pow(cosA,2) - Math.pow(sinA,2));
      }
      
      override protected function updatePosition() : void
      {
         if(!enabled)
         {
            return;
         }
         if(this.golow_tx)
         {
            (this.golow_tx._progressTx as TextField).autoSize = TextFieldAutoSize.RIGHT;
            this.golow_tx._progressTx.text = _value + "/" + _maximum;
         }
         var barSize:Number = this._size;
         var ratio:Number;
         if(this._progress)
         {
            ratio = (_value - _minimum - this._progress) / (_maximum - _minimum);
         }
         else
         {
            ratio = (_value - _minimum - this._progressShift) / (_maximum - _minimum);
         }
         if(this.filled)
         {
            this._mask.width = this.filled.width * ratio;
         }
         if(this._progressShift)
         {
            this._mask_new.x = barSize * ratio;
            ratio = (this._progressShift - _minimum) / (_maximum - _minimum);
            this._mask_new.width = this.filled.width / this.filled.scaleX * ratio;
         }
      }
   }
}

