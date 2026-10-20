package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.menu.ActionMenu;
   
   public dynamic class SrvActionMenu extends ActionMenu
   {
      
      public function SrvActionMenu()
      {
         super();
         this.__setProp_menu_buttons_SrvActionMenu_Layer1_0();
      }
      
      internal function __setProp_menu_buttons_SrvActionMenu_Layer1_0() : *
      {
         try
         {
            menu_buttons["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         menu_buttons.autoSize = "none";
         menu_buttons.buttonWidth = 170;
         menu_buttons.direction = "vertical";
         menu_buttons.enabled = true;
         menu_buttons.enableInitCallback = false;
         menu_buttons.focusable = true;
         menu_buttons.itemRendererName = "SrvButtonV5Ch";
         menu_buttons.spacing = 0;
         menu_buttons.visible = true;
         try
         {
            menu_buttons["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

