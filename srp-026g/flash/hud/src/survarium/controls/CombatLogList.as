package survarium.controls
{
   import flash.display.MovieClip;
   
   public class CombatLogList extends MovieClip
   {
      
      protected var _renderers:Array = new Array();
      
      protected var _itemHeight:uint = 30;
      
      public function CombatLogList()
      {
         super();
      }
      
      override public function get width() : Number
      {
         return 200;
      }
      
      public function clear() : void
      {
         while(this._renderers.length)
         {
            this.delItem();
         }
      }
      
      public function addItem(item:Object) : void
      {
         var itemRenderer:MovieClip = new SrvCombatLogRenderer();
         itemRenderer.setData(item,this);
         this._renderers.push(itemRenderer);
         addChild(itemRenderer);
         this.validateRenderers();
      }
      
      public function delItem() : void
      {
         var i:int = 0;
         if(!this._renderers[i])
         {
            return;
         }
         this._renderers[i].clearEvents();
         removeChild(this._renderers[i]);
         this._renderers.splice(i,1);
         this.validateRenderers();
      }
      
      override public function toString() : String
      {
         return "[CombatLogList " + name + "]";
      }
      
      public function validateRenderers() : void
      {
         var l:uint = this._renderers.length;
         var h:Number = this._itemHeight;
         var availableHeight:uint = this.height;
         var totalItems:uint = availableHeight / h;
         var startPoint:Number;
         if(l > totalItems)
         {
            startPoint = 0;
            this.delItem();
            l = this._renderers.length;
         }
         else
         {
            startPoint = (totalItems - l) * h;
         }
         for(var i:uint = 0; i < l; i++)
         {
            var renderer:CombatLogRenderer;
            renderer = this._renderers[i];
            renderer.x = 0;
            renderer.y = i * h;
         }
      }
   }
}

