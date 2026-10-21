package com.greensock.easing
{
   public final class BackIn extends Ease
   {
      
      public static var ease:BackIn = new BackIn();
      
      public function BackIn(overshoot:Number = 1.70158)
      {
         super();
         _p1 = overshoot;
      }
      
      override public function getRatio(progress:Number) : Number
      {
         return progress * progress * ((_p1 + 1) * progress - _p1);
      }
      
      public function config(overshoot:Number = 1.70158) : BackIn
      {
         return new BackIn(overshoot);
      }
   }
}

