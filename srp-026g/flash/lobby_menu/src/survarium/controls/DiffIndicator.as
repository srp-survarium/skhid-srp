package survarium.controls
{
   import flash.display.Sprite;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.StatusIndicator;
   import scaleform.clik.motion.Tween;
   import survarium.constants.PropertyDirection;
   
   // A card's stat row: the bar runs over the property's min_value..max_value (ui_properties) and
   // stops at its ends; value and compare keep what they were given, so the text, the difference
   // and the upgrade preview read the real numbers (a VSSK's 175 damage over the scale's 150).
   public class DiffIndicator extends StatusIndicator
   {
      
      public var mc_diff_negative:Sprite;
      
      public var mc_diff_positive:Sprite;
      
      public var mc_diff_positive_ghost:Sprite;
      
      public var mc_diff_negative_ghost:Sprite;
      
      public var mc_value:Sprite;
      
      protected var _otherValue:Number = 0;
      
      // value and compare as given; _value and _otherValue are them within the bar's range
      protected var _realValue:Number = 0;
      
      protected var _realOther:Number = 0;
      
      protected var _upgrade:Number = 0;
      
      protected var _valueDiff:Number = 0;
      
      protected var _realDiff:Number = 0;
      
      protected var _extraValue:Number = 0;
      
      protected var _size:Number;
      
      protected var _tween:Tween;
      
      protected var _direction:Boolean;
      
      public function DiffIndicator()
      {
         super();
         _size = this.mc_value.width;
         this.mc_value.width = 0;
         this.mc_diff_positive.width = 0;
         this.mc_diff_negative.width = 0;
         if(this.mc_diff_positive_ghost && this.mc_diff_negative_ghost)
         {
            this.mc_diff_positive_ghost.width = 0;
            this.mc_diff_negative_ghost.width = 0;
         }
      }
      
      public function set upgrade(value:Number) : void
      {
         _upgrade = value;
         invalidate(InvalidationType.DATA);
      }
      
      public function get upgrade() : Number
      {
         return this._upgrade;
      }
      
      public function set extra(value:Number) : void
      {
         _extraValue = value;
         invalidate(InvalidationType.DATA);
      }
      
      public function get extra() : Number
      {
         return this._extraValue;
      }
      
      override public function get value() : Number
      {
         return this._realValue;
      }
      
      override public function set value(value:Number) : void
      {
         if(this._realValue == value && _value == this.inRange(value))
         {
            return;
         }
         _realValue = value;
         _value = this.inRange(value);
         this.updatePosition();
      }
      
      public function set compare(value:Number) : void
      {
         _realOther = value;
         _otherValue = this.inRange(value);
         invalidate(InvalidationType.DATA);
      }
      
      protected function inRange(value:Number) : Number
      {
         return Math.max(_minimum,Math.min(_maximum,value));
      }
      
      // A difference bar starts where the value bar ends; it ends at the bar's end at most.
      protected function fitBar(bar:Sprite) : void
      {
         bar.width = Math.max(0,Math.min(bar.width,this.mc_value.x + this._size - bar.x));
      }
      
      public function set direction(value:Boolean) : void
      {
         _direction = value;
      }
      
      override protected function updatePosition() : void
      {
         var ratio:Number;
         var diffBar:Sprite;
         if(!enabled)
         {
            return;
         }
         _valueDiff = this._otherValue - _value;
         _realDiff = this._realOther - this._realValue;
         if(this._valueDiff > 0)
         {
            ratio = (_value - _minimum) / (_maximum - _minimum);
         }
         else
         {
            ratio = (_value - _minimum + this._valueDiff) / (_maximum - _minimum);
         }
         this.mc_value.width = this._size * Math.max(0,ratio);
         if(this.mc_diff_positive_ghost && this.mc_diff_negative_ghost)
         {
            if(this._extraValue == 0 || this._valueDiff != 0)
            {
               this.mc_diff_positive_ghost.width = 0;
               this.mc_diff_negative_ghost.width = 0;
            }
            else
            {
               if(this._direction)
               {
                  if(this._extraValue > 0)
                  {
                     this.mc_diff_negative_ghost.width = 0;
                     diffBar = this.mc_diff_positive_ghost;
                  }
                  else
                  {
                     this.mc_diff_positive_ghost.width = 0;
                     diffBar = this.mc_diff_negative_ghost;
                  }
               }
               else if(this._extraValue > 0)
               {
                  this.mc_diff_positive_ghost.width = 0;
                  diffBar = this.mc_diff_negative_ghost;
               }
               else
               {
                  this.mc_diff_negative_ghost.width = 0;
                  diffBar = this.mc_diff_positive_ghost;
               }
               if(this._upgrade == 0 && (diffBar == this.mc_diff_positive_ghost && this._direction == PropertyDirection.HIGHER_IS_BETTER || diffBar == this.mc_diff_negative_ghost && this._direction == PropertyDirection.LOWER_IS_BETTER))
               {
                  diffBar.width = 0;
               }
               else
               {
                  if(this._extraValue > 0)
                  {
                     if(this._valueDiff > 0)
                     {
                        ratio = (_value - _minimum - this._extraValue) / (_maximum - _minimum);
                     }
                     else
                     {
                        ratio = (_value - _minimum + this._valueDiff - this._extraValue) / (_maximum - _minimum);
                     }
                     this.mc_value.width = this._size * Math.max(0,ratio);
                  }
                  diffBar.x = this.mc_value.width + 1 + this.mc_value.x;
                  ratio = Math.abs(this._extraValue - _minimum) / (_maximum - _minimum);
                  diffBar.width = (this._size - 1) * ratio;
                  this.fitBar(diffBar);
               }
            }
         }
         if(this._upgrade != 0 && this._valueDiff == 0 && this._extraValue != 0)
         {
            trace("c",this._extraValue,_value);
            if(this._direction)
            {
               if(this._upgrade > 0)
               {
                  this.mc_diff_negative.width = 0;
                  diffBar = this.mc_diff_positive;
               }
               else
               {
                  this.mc_diff_positive.width = 0;
                  diffBar = this.mc_diff_negative;
               }
            }
            else if(this._upgrade > 0)
            {
               this.mc_diff_positive.width = 0;
               diffBar = this.mc_diff_negative;
            }
            else
            {
               this.mc_diff_negative.width = 0;
               diffBar = this.mc_diff_positive;
            }
            ratio = Math.abs(this._upgrade - _minimum) / (_maximum - _minimum);
            diffBar.width = (this._size - 1) * ratio;
            diffBar.x = this.mc_value.width + 1 + this.mc_value.x;
            this.fitBar(diffBar);
         }
         else if(this._valueDiff == 0)
         {
            this.mc_diff_positive.width = 0;
            this.mc_diff_negative.width = 0;
         }
         else
         {
            if(this._direction)
            {
               if(this._valueDiff > 0)
               {
                  this.mc_diff_negative.width = 0;
                  diffBar = this.mc_diff_positive;
               }
               else
               {
                  this.mc_diff_positive.width = 0;
                  diffBar = this.mc_diff_negative;
               }
            }
            else if(this._valueDiff > 0)
            {
               this.mc_diff_positive.width = 0;
               diffBar = this.mc_diff_negative;
            }
            else
            {
               this.mc_diff_negative.width = 0;
               diffBar = this.mc_diff_positive;
            }
            diffBar.x = this.mc_value.width + 1 + this.mc_value.x;
            ratio = Math.abs(this._valueDiff - _minimum) / (_maximum - _minimum);
            diffBar.width = (this._size - 1) * ratio;
            this.fitBar(diffBar);
         }
      }
   }
}
      
