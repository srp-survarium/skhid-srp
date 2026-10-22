package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.DirectionMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.constants.WrappingMode;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
   import scaleform.clik.ui.InputDetails;
   
   public class FilterList extends TileList
   {
      
      public static const NEW_DATA_PROVIDER:String = "new_data_provider";
      
      public var _body:MovieClip;
      
      protected var _fncCallBack:Function;
      
      protected var _maxCols:int = 5;
      
      protected var _selected:Array = new Array();
      
      protected var _providers:Array = new Array();
      
      protected var _focusIndex:int = 1;
      
      protected var _partDataMode:Boolean = false;
      
      public function FilterList()
      {
         super();
         addEventListener(MouseEvent.MOUSE_DOWN,this.inFocusM6);
      }
      
      public function registerCallBack(callback:Function) : void
      {
         _fncCallBack = callback;
      }
      
      public function registerDataProvider(providerId:uint, provider:DataProvider) : void
      {
         this._providers[providerId] = provider;
      }
      
      public function getDataProviderCount() : uint
      {
         return this._providers.length;
      }
      
      public function getDataProvider(providerId:uint) : DataProvider
      {
         return this._providers[providerId];
      }
      
      public function clear_providers() : void
      {
         this._providers.length = 0;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      protected function inFocusM6(event:MouseEvent) : void
      {
         focusIndex = -1;
      }
      
      override protected function changeFocus() : void
      {
         if(!_focused)
         {
            focusIndex = -1;
         }
         else if(selectedIndex >= dataProvider.length || selectedIndex == -1)
         {
            focusIndex = 0;
         }
         else
         {
            focusIndex = selectedIndex;
         }
      }
      
      public function setDataProvider(providerId:uint) : void
      {
         var i:uint;
         if(dataProvider == this._providers[providerId])
         {
            return;
         }
         _partDataMode = false;
         dataProvider = this._providers[providerId];
         if(dataProvider)
         {
            i = 0;
            while(i < dataProvider.length)
            {
               dataProvider[i].enabled = true;
               i++;
            }
         }
         dispatchEvent(new Event(NEW_DATA_PROVIDER));
      }
      
      public function set focusIndex(value:int) : void
      {
         if(value == this._focusIndex)
         {
            return;
         }
         if(!_renderers || _renderers.length == 0)
         {
            return;
         }
         var lastFocusIndex:int = this._focusIndex;
         var renderer:Button = _renderers[lastFocusIndex] as Button;
         if(renderer)
         {
            renderer.displayFocus = false;
         }
         _focusIndex = value;
         renderer = _renderers[this._focusIndex] as Button;
         if(renderer)
         {
            renderer.displayFocus = true;
         }
      }
      
      override public function handleInput(event:InputEvent) : void
      {
         if(event.handled)
         {
            return;
         }
         var renderer:IListItemRenderer = getRendererAt(this._focusIndex,_scrollPosition);
         if(renderer != null)
         {
            renderer.handleInput(event);
            if(event.handled)
            {
               return;
            }
         }
         var details:InputDetails = event.details;
         var keyPress:Boolean = details.value == InputValue.KEY_DOWN || details.value == InputValue.KEY_HOLD;
         var nextIndex:int = -1;
         var nav:String = details.navEquivalent;
         if(_direction == DirectionMode.HORIZONTAL)
         {
            switch(nav)
            {
               case NavigationCode.RIGHT:
                  nextIndex = this._focusIndex + _totalRows;
                  break;
               case NavigationCode.LEFT:
                  nextIndex = this._focusIndex - _totalRows;
                  break;
               case NavigationCode.UP:
                  nextIndex = this._focusIndex - 1;
                  break;
               case NavigationCode.DOWN:
                  nextIndex = this._focusIndex + 1;
               break;
            }
         }
         else
         {
            switch(nav)
            {
               case NavigationCode.DOWN:
                  nextIndex = this._focusIndex + _totalColumns;
                  break;
               case NavigationCode.UP:
                  nextIndex = this._focusIndex - _totalColumns;
                  break;
               case NavigationCode.LEFT:
                  nextIndex = this._focusIndex - 1;
                  break;
               case NavigationCode.RIGHT:
                  nextIndex = this._focusIndex + 1;
               break;
            }
         }
         if(nextIndex == -1)
         {
            switch(nav)
            {
               case NavigationCode.HOME:
                  nextIndex = 0;
                  break;
               case NavigationCode.END:
                  nextIndex = _dataProvider.length - 1;
                  break;
               case NavigationCode.PAGE_DOWN:
                  nextIndex = Math.min(_dataProvider.length - 1,this._focusIndex + _totalColumns * _totalRows);
                  break;
               case NavigationCode.PAGE_UP:
                  nextIndex = Math.max(0,this._focusIndex - _totalColumns * _totalRows);
               break;
            }
         }
         if(nextIndex != -1)
         {
            if(!keyPress)
            {
               event.handled = true;
               return;
            }
            if(nextIndex >= 0 && nextIndex < dataProvider.length)
            {
               focusIndex = Math.max(0,Math.min(_dataProvider.length - 1,nextIndex));
               event.handled = true;
            }
            else if(wrapping == WrappingMode.STICK)
            {
               nextIndex = Math.max(0,Math.min(_dataProvider.length - 1,nextIndex));
               if(this._focusIndex != nextIndex)
               {
                  focusIndex = nextIndex;
               }
               event.handled = true;
            }
            else if(wrapping == WrappingMode.WRAP)
            {
               focusIndex = nextIndex < 0 ? (_dataProvider.length - 1) : (this._focusIndex < _dataProvider.length - 1 ? (_dataProvider.length - 1) : 0);
               event.handled = true;
            }
         }
      }
      
      public function getSelectedPositions() : Array
      {
         var positions:Array = [];
         var i:uint;
         if(dataProvider)
         {
            i = 0;
            while(i < dataProvider.length)
            {
               if(dataProvider[i].selected == true)
               {
                  positions.push(i);
               }
               i++;
            }
         }
         return positions;
      }
      
      public function getSelectedIndexes() : Array
      {
         var count:uint;
         var item:Object;
         var i:uint;
         this._selected.length = 0;
         if(dataProvider)
         {
            count = dataProvider.length;
            i = 0;
            while(i < count)
            {
               item = dataProvider[i];
               if(item.selected == true)
               {
                  this._selected.push(item.sucat);
               }
               i++;
            }
            trace("getSelectedIndexes",this._partDataMode,this._selected.length);
            if(this._partDataMode && this._selected.length == 0)
            {
               i = 0;
               while(i < count)
               {
                  item = dataProvider[i];
                  trace("iterateData::",item.enabled);
                  if(item.enabled != false)
                  {
                     this._selected.push(item.sucat);
                  }
                  i++;
               }
            }
         }
         return this._selected;
      }
      
      override public function set selectedIndex(value:int) : void
      {
         _newSelectedIndex = value;
         invalidateSelectedIndex();
         focusIndex = value;
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
      
      override protected function refreshData() : void
      {
         var itemsPerSet:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var totalSets:Number = Math.ceil(_dataProvider.length / itemsPerSet);
         var maxScrollPosition:Number = totalSets - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows);
         _scrollPosition = Math.max(0,Math.min(maxScrollPosition,_scrollPosition));
         var startIndex:Number = _scrollPosition * itemsPerSet;
         var endIndex:Number = startIndex + _totalColumns * _totalRows - 1;
         _dataProvider.requestItemRange(startIndex,endIndex,populateData);
      }
      
      public function focusPart(positions:Array, providerId:uint = 0) : void
      {
         trace("focusPart",positions,"id:",providerId,_selectedIndex);
         _partDataMode = true;
         var i:uint = 0;
         while(i < this._providers[providerId].length)
         {
            this._providers[providerId][i].selected = false;
            this._providers[providerId][i].enabled = false;
            i++;
         }
         i = 0;
         while(i < positions.length)
         {
            this._providers[providerId][positions[i]].selected = true;
            this._providers[providerId][positions[i]].enabled = true;
            i++;
         }
         this.refreshData();
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
      
      public function selectPart(positions:Array, providerId:uint = 0, unused:Boolean = false) : void
      {
         trace("selectPart",positions,"id:",providerId,_selectedIndex);
         var i:uint = 0;
         while(i < this._providers[providerId].length)
         {
            this._providers[providerId][i].selected = false;
            i++;
         }
         i = 0;
         while(i < positions.length)
         {
            this._providers[providerId][positions[i]].selected = true;
            i++;
         }
         this.refreshData();
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
      
      public function deselectAll() : void
      {
         trace("deselectAll");
         var i:uint = 0;
         var renderer:IListItemRenderer;
         while(i < _renderers.length)
         {
            renderer = getRendererAt(i,scrollPosition);
            renderer.selected = false;
            i++;
         }
      }
      
      public function selectAll() : void
      {
         trace("selectAll");
         var i:uint = 0;
         var renderer:IListItemRenderer;
         while(i < _renderers.length)
         {
            renderer = getRendererAt(i,scrollPosition);
            renderer.selected = true;
            renderer.validateNow();
            i++;
         }
      }
      
      override protected function updateSelectedIndex() : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var renderer:IListItemRenderer = getRendererAt(_selectedIndex,scrollPosition);
         if(renderer != null)
         {
            renderer.validateNow();
         }
         _selectedIndex = _newSelectedIndex;
         invalidateSelectedIndex();
         if(_selectedIndex < 0 || _selectedIndex >= _dataProvider.length)
         {
            return;
         }
         renderer = getRendererAt(_selectedIndex,_scrollPosition);
         if(renderer != null)
         {
            renderer.selected = !renderer.selected;
            renderer.validateNow();
         }
         else
         {
            scrollToIndex(_selectedIndex);
            renderer = getRendererAt(_selectedIndex,scrollPosition);
            renderer.selected = !renderer.selected;
            renderer.validateNow();
         }
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
   }
}

