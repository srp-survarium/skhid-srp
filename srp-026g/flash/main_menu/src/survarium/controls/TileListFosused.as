package survarium.controls
{
   import flash.events.MouseEvent;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public class TileListFosused extends TileList
   {
      
      public var overIndex:int;
      
      protected var _lastIndex:int = -1;
      
      public function TileListFosused()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         addEventListener(MouseEvent.MOUSE_OVER,this.newIndex);
         addEventListener(MouseEvent.MOUSE_OUT,this.newIndex);
      }
      
      protected function newIndex(event:MouseEvent) : void
      {
         if(event.target is IListItemRenderer)
         {
            overIndex = event.target.index;
         }
         if(event.type == MouseEvent.MOUSE_OUT)
         {
            overIndex = -1;
         }
      }
      
      override protected function changeFocus() : void
      {
         var skipFocus:Boolean = false;
         var lastRenderer:IListItemRenderer;
         var renderer:IListItemRenderer;
         if(focused > 0 && this.overIndex != -1)
         {
            renderer = getRendererAt(this.overIndex,_scrollPosition);
            _selectedIndex = this.overIndex;
            if(this._lastIndex != -1 && this.overIndex != this._lastIndex)
            {
               lastRenderer = getRendererAt(this._lastIndex,_scrollPosition);
            }
         }
         else
         {
            renderer = getRendererAt(_selectedIndex,_scrollPosition);
         }
         if(lastRenderer != null)
         {
            lastRenderer.selected = false;
            lastRenderer.displayFocus = false;
            lastRenderer.validateNow();
         }
         if(renderer != null)
         {
            if(focused > 0 && !skipFocus)
            {
               renderer.displayFocus = true;
               renderer.selected = true;
               renderer.validateNow();
            }
            else
            {
               if(!skipFocus)
               {
                  _lastIndex = renderer.index;
               }
               renderer.selected = false;
               renderer.displayFocus = false;
               renderer.validateNow();
            }
         }
      }
   }
}

