package
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public dynamic class SymbolFontsExo extends MovieClip
   {
      
      public var textField:TextField;
      
      public var i:uint;
      
      public var tx:TextField;
      
      public function SymbolFontsExo()
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

