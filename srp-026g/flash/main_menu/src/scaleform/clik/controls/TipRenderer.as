package scaleform.clik.controls
{
   import flash.display.MovieClip;
   import scaleform.clik.interfaces.ITipRenderer;
   
   public class TipRenderer extends MovieClip implements ITipRenderer
   {
      
      protected var _data:Object;
      
      public function TipRenderer()
      {
         super();
      }
      
      public function get data() : Object
      {
         return this._data;
      }
      
      public function set data(value:Object) : void
      {
         if(this._data == value)
         {
            return;
         }
         _data = value;
         this.invalidateData();
      }
      
      protected function invalidateData() : void
      {
      }
   }
}

