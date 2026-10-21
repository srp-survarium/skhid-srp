package survarium.controls
{
   import flash.display.Sprite;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.StatusIndicator;
   import scaleform.clik.motion.Tween;
   
   public class DiffIndicator extends StatusIndicator
   {
      
      public var mc_diff_negative:Sprite;
      
      public var mc_diff_positive:Sprite;
      
      public var mc_diff_positive_ghost:Sprite;
      
      public var mc_diff_negative_ghost:Sprite;
      
      public var mc_value:Sprite;
      
      protected var _otherValue:Number = 0;
      
      protected var _upgrade:Number = 0;
      
      protected var _valueDiff:Number = 0;
      
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
      
      public function set upgrade(param1:Number) : void
      {
         _upgrade = param1;
         invalidate(InvalidationType.DATA);
      }
      
      public function get upgrade() : Number
      {
         return this._upgrade;
      }
      
      public function set extra(param1:Number) : void
      {
         _extraValue = param1;
         invalidate(InvalidationType.DATA);
      }
      
      public function get extra() : Number
      {
         return this._extraValue;
      }
      
      public function set compare(param1:Number) : void
      {
         _otherValue = param1;
         if(this._otherValue > _maximum)
         {
            _otherValue = _maximum;
         }
         if(this._otherValue < _minimum)
         {
            _otherValue = _minimum;
         }
         invalidate(InvalidationType.DATA);
      }
      
      public function set direction(param1:Boolean) : void
      {
         _direction = param1;
      }
      
      override protected function updatePosition() : void
      {
         var _loc1_:Number;
         var _loc2_:Sprite;
         if(!enabled)
         {
            return;
         }
         _valueDiff = this._otherValue - _value;
         if(this._valueDiff > 0)
         {
            _loc1_ = (_value - _minimum) / (_maximum - _minimum);
         }
         else
         {
            _loc1_ = (_value - _minimum + this._valueDiff) / (_maximum - _minimum);
         }
         this.mc_value.width = this._size * _loc1_;
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
                     _loc2_ = this.mc_diff_positive_ghost;
                  }
                  else
                  {
                     this.mc_diff_positive_ghost.width = 0;
                     _loc2_ = this.mc_diff_negative_ghost;
                  }
               }
               else if(this._extraValue > 0)
               {
                  this.mc_diff_positive_ghost.width = 0;
                  _loc2_ = this.mc_diff_negative_ghost;
               }
               else
               {
                  this.mc_diff_negative_ghost.width = 0;
                  _loc2_ = this.mc_diff_positive_ghost;
               }
               if(this._upgrade == 0 && (_loc2_ == this.mc_diff_positive_ghost && this._direction == 1 || _loc2_ == this.mc_diff_negative_ghost && this._direction == 0))
               {
                  _loc2_.width = 0;
               }
               else
               {
                  if(this._extraValue > 0)
                  {
                     if(this._valueDiff > 0)
                     {
                        _loc1_ = (_value - _minimum - this._extraValue) / (_maximum - _minimum);
                     }
                     else
                     {
                        _loc1_ = (_value - _minimum + this._valueDiff - this._extraValue) / (_maximum - _minimum);
                     }
                     this.mc_value.width = this._size * _loc1_;
                  }
                  _loc2_.x = this.mc_value.width + 1 + this.mc_value.x;
                  _loc1_ = Math.abs(this._extraValue - _minimum) / (_maximum - _minimum);
                  _loc2_.width = (this._size - 1) * _loc1_;
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
                  _loc2_ = this.mc_diff_positive;
               }
               else
               {
                  this.mc_diff_positive.width = 0;
                  _loc2_ = this.mc_diff_negative;
               }
            }
            else if(this._upgrade > 0)
            {
               this.mc_diff_positive.width = 0;
               _loc2_ = this.mc_diff_negative;
            }
            else
            {
               this.mc_diff_negative.width = 0;
               _loc2_ = this.mc_diff_positive;
            }
            _loc1_ = Math.abs(this._upgrade - _minimum) / (_maximum - _minimum);
            _loc2_.width = (this._size - 1) * _loc1_;
            _loc2_.x = this.mc_value.width + 1 + this.mc_value.x;
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
                  _loc2_ = this.mc_diff_positive;
               }
               else
               {
                  this.mc_diff_positive.width = 0;
                  _loc2_ = this.mc_diff_negative;
               }
            }
            else if(this._valueDiff > 0)
            {
               this.mc_diff_positive.width = 0;
               _loc2_ = this.mc_diff_negative;
            }
            else
            {
               this.mc_diff_negative.width = 0;
               _loc2_ = this.mc_diff_positive;
            }
            _loc2_.x = this.mc_value.width + 1 + this.mc_value.x;
            _loc1_ = Math.abs(this._valueDiff - _minimum) / (_maximum - _minimum);
            _loc2_.width = (this._size - 1) * _loc1_;
         }
      }
   }
}

