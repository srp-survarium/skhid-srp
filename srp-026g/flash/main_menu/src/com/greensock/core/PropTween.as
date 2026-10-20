package com.greensock.core
{
   public final class PropTween
   {
      
      public var t:Object;
      
      public var p:String;
      
      public var s:Number;
      
      public var c:Number;
      
      public var f:Boolean;
      
      public var pr:int;
      
      public var pg:Boolean;
      
      public var n:String;
      
      public var r:Boolean;
      
      public var _next:PropTween;
      
      public var _prev:PropTween;
      
      public function PropTween(param1:Object, param2:String, param3:Number, param4:Number, param5:String, param6:Boolean, param7:PropTween = null, param8:int = 0)
      {
         super();
         t = param1;
         p = param2;
         s = param3;
         c = param4;
         n = param5;
         f = param1[param2] is Function;
         pg = param6;
         if(param7)
         {
            param7._prev = this;
            _next = param7;
         }
         pr = param8;
      }
   }
}

