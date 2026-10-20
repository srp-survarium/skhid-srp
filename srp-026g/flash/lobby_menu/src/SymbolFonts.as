package
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public dynamic class SymbolFonts extends MovieClip
   {
      
      public var tx1:TextField;
      
      public var tx2:TextField;
      
      public var textPoints:TextField;
      
      public var textField2:TextField;
      
      public var tx3:TextField;
      
      public var tx4:TextField;
      
      public var tx5:TextField;
      
      public var tx6:TextField;
      
      public var textField4:TextField;
      
      public var textField:TextField;
      
      public var textField5:TextField;
      
      public var i:uint;
      
      public var tx:TextField;
      
      public function SymbolFonts()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         i = 0;
         while(this.i < numChildren)
         {
            tx = getChildAt(this.i) as TextField;
            if(this.tx)
            {
               this.tx.text = "Abcdefg124 Абцдфг 1231";
            }
            ++this.i;
         }
      }
   }
}

