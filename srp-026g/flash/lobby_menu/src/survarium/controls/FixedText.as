package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.Sprite;
   import flash.events.TimerEvent;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import flash.utils.Timer;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.motion.Tween;
   import scaleform.clik.utils.Constraints;
   
   public class FixedText extends Sprite
   {
      
      public var constraints:Constraints;
      
      public var textField:TextField;
      
      protected var _times:int = 0;
      
      protected var _mask:Sprite = new Sprite();
      
      protected var _timer:Timer = new Timer(3000);
      
      protected var _pos:int = 0;
      
      protected var _side:Boolean = true;
      
      protected var _text:String = "";
      
      protected var _tween:Tween;
      
      protected var _sw:Number;
      
      protected var _mw:Number;
      
      protected var _resize:Boolean = false;
      
      protected var _tweenLite:TweenLite;
      
      public function FixedText()
      {
         super();
         _text = this.textField.text;
         constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         this.constraints.addElement("textField",this.textField,Constraints.LEFT);
         var _loc1_:TextFormat = this.textField.getTextFormat();
         if(_loc1_.align == TextFormatAlign.CENTER)
         {
            _resize = true;
         }
         else
         {
            this.textField.autoSize = "left";
         }
         this.refresh();
         addChild(this._mask);
         this.mask = this._mask;
         this._timer.addEventListener(TimerEvent.TIMER,this.onTimer);
      }
      
      protected function refresh() : void
      {
         _mw = this.width / scaleX;
         this._mask.graphics.clear();
         this._mask.graphics.beginFill(0);
         this._mask.graphics.drawRect(0,0,this._mw,height);
         this._mask.graphics.endFill();
         _sw = this.width;
         this.constraints.update(this.width,height);
      }
      
      override public function get width() : Number
      {
         return super.width;
      }
      
      override public function set width(param1:Number) : void
      {
         if(this._sw != param1)
         {
            super.width = param1;
            this.refresh();
         }
      }
      
      public function getWidth() : int
      {
         var _loc1_:int = this.textField.textWidth < this._mw ? (this.textField.textWidth) : this._mw;
         return _loc1_;
      }
      
      public function get shift() : int
      {
         return this._pos;
      }
      
      public function set shift(param1:int) : void
      {
         _pos = param1;
         if(this.textField)
         {
            this.textField.x = this.shift;
         }
      }
      
      [Inspectable(defaultValue="defaultText")]
      public function get text() : String
      {
         return this._text;
      }
      
      public function set text(param1:String) : void
      {
         if(this._text != param1)
         {
            this.textField.text = param1;
            if(this._resize)
            {
               if(this.textField.textWidth > this._sw)
               {
                  this.textField.width = this.textField.textWidth + 3;
               }
               else
               {
                  this.textField.width = this._sw;
               }
            }
            _text = param1;
            if(this.textField.textWidth < this._sw)
            {
               if(this.mask)
               {
                  this.mask = null;
                  removeChild(this._mask);
               }
               if(this._timer.running)
               {
                  this._timer.stop();
               }
               if(this._tweenLite)
               {
                  this._tweenLite.kill();
                  _tweenLite = null;
               }
               shift = 1;
            }
            else
            {
               if(!mask)
               {
                  this.mask = this._mask;
                  addChild(this._mask);
               }
               this._timer.reset();
               this._timer.start();
               if(this._tweenLite)
               {
                  this._tweenLite.kill();
                  _tweenLite = null;
               }
               _side = true;
               shift = 1;
            }
         }
      }
      
      protected function onTimer(param1:TimerEvent) : void
      {
         if(!this.textField)
         {
            return;
         }
         if(this._side)
         {
            if(this._tweenLite)
            {
               this._tweenLite.kill();
            }
            _tweenLite = TweenLite.to(this,1,{shift:this._mask.width - this.textField.width - 1});
         }
         else
         {
            if(this._tweenLite)
            {
               this._tweenLite.kill();
            }
            _tweenLite = TweenLite.to(this,0.5,{shift:1});
         }
         _side = !this._side;
      }
   }
}

