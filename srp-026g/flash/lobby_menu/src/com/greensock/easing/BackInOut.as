package com.greensock.easing
{
   public final class BackInOut extends Ease
   {
      
      public static var ease:BackInOut = new BackInOut();
      
      public function BackInOut(overshoot:Number = 1.70158)
      {
         super();
         _p1 = overshoot;
         _p2 = _p1 * 1.525;
      }
      
      override public function getRatio(progress:Number) : Number
      {
         return (progress = progress * 2) < 1 ? 0.5 * progress * progress * ((_p2 + 1) * progress - _p2) : 0.5 * ((progress = progress - 2) * progress * ((_p2 + 1) * progress + _p2) + 2);
      }
      
      public function config(overshoot:Number = 1.70158) : BackInOut
      {
         return new BackInOut(overshoot);
      }
   }
}

